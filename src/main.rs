    use anyhow::{Context, Result};
use clap::Parser;
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use regex::Regex;
use once_cell::sync::Lazy;

/// HelloFresh recipe scraper
#[derive(Parser, Debug)]
#[command(name = "hellofresh-scraper")]
#[command(about = "Scrapes recipes from HelloFresh sitemaps using cooklang-import")]
struct Args {
    /// Countries to scrape (e.g., ie, de, us). If not specified, scrapes all countries.
    #[arg(short, long, value_delimiter = ',')]
    countries: Option<Vec<String>>,

    /// Output directory for recipes
    #[arg(short, long, default_value = "recipes")]
    output: PathBuf,

    /// Path to cooklang-import binary
    #[arg(long, default_value = "../cooklang-import/target/debug/cooklang-import")]
    cooklang_import: PathBuf,

    /// Maximum concurrent downloads
    #[arg(long, default_value = "5")]
    concurrency: usize,

    /// List available countries and exit
    #[arg(long)]
    list_countries: bool,

    /// Limit number of recipes per country (for testing)
    #[arg(long)]
    limit: Option<usize>,

    /// Skip recipes that already exist
    #[arg(long, default_value = "true")]
    skip_existing: bool,
}

/// HelloFresh country configuration
#[derive(Debug, Clone)]
struct Country {
    code: &'static str,
    domain: &'static str,
    name: &'static str,
}

impl Country {
    const fn new(code: &'static str, domain: &'static str, name: &'static str) -> Self {
        Self { code, domain, name }
    }

    fn sitemap_url(&self) -> String {
        format!("https://www.hellofresh.{}/sitemap_recipe_pages.xml", self.domain)
    }
}

/// All HelloFresh countries
fn get_countries() -> HashMap<&'static str, Country> {
    let countries = vec![
        Country::new("at", "at", "Austria"),
        Country::new("au", "com.au", "Australia"),
        Country::new("be", "be", "Belgium"),
        Country::new("ca", "ca", "Canada"),
        Country::new("ch", "ch", "Switzerland"),
        Country::new("de", "de", "Germany"),
        Country::new("dk", "dk", "Denmark"),
        Country::new("es", "es", "Spain"),
        Country::new("fr", "fr", "France"),
        Country::new("gb", "co.uk", "United Kingdom"),
        Country::new("ie", "ie", "Ireland"),
        Country::new("it", "it", "Italy"),
        Country::new("lu", "lu", "Luxembourg"),
        Country::new("nl", "nl", "Netherlands"),
        Country::new("no", "no", "Norway"),
        Country::new("nz", "co.nz", "New Zealand"),
        Country::new("se", "se", "Sweden"),
        Country::new("us", "com", "United States"),
    ];

    countries.into_iter().map(|c| (c.code, c)).collect()
}

/// Parse recipe URLs from sitemap XML
async fn fetch_sitemap_urls(client: &reqwest::Client, sitemap_url: &str) -> Result<Vec<String>> {
    info!("Fetching sitemap: {}", sitemap_url);

    let response = client
        .get(sitemap_url)
        .send()
        .await
        .context("Failed to fetch sitemap")?;

    if !response.status().is_success() {
        anyhow::bail!("Sitemap returned status: {}", response.status());
    }

    let xml = response.text().await.context("Failed to read sitemap body")?;
    parse_sitemap_urls(&xml)
}

/// Parse URLs from sitemap XML content
fn parse_sitemap_urls(xml: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut urls = Vec::new();
    let mut in_loc = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"loc" => {
                in_loc = true;
            }
            Ok(Event::Text(e)) if in_loc => {
                let url = e.unescape()?.to_string();
                if url.contains("/recipes/") {
                    urls.push(url);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"loc" => {
                in_loc = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("XML parse error: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("Found {} recipe URLs in sitemap", urls.len());
    Ok(urls)
}

/// Extract recipe slug from URL for filename
fn url_to_filename(url: &str) -> Option<String> {
    // URL format: https://www.hellofresh.ie/recipes/recipe-name-abc123
    url.split("/recipes/")
        .nth(1)
        .map(|s| format!("{}.recipe", s))
}

// Regex to strip HTML tags
static HTML_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<[^>]+>").unwrap()
});

/// Strip HTML tags and clean up content
fn strip_html_tags(content: &str) -> String {
    // Remove all HTML tags
    let result = HTML_TAG_RE.replace_all(content, "").to_string();

    // Replace non-breaking spaces with regular spaces
    let result = result.replace('\u{00A0}', " ");

    // Collapse multiple blank lines into single blank lines
    let mut result = result;
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    // Trim trailing whitespace from lines (preserve leading indentation for YAML)
    result
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clean up the recipe content - remove incomplete image URLs and fix formatting
fn clean_recipe_content(content: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();

    // Remove lines with incomplete/empty image URLs
    lines.retain(|line| {
        let trimmed = line.trim();
        // Keep lines that are not incomplete image URLs
        // Incomplete URLs end with just the transformation params and no actual image path
        if trimmed.starts_with("image:") {
            let url_part = trimmed.strip_prefix("image:").unwrap_or("").trim();
            // Valid image URLs should have a path after the transformation params
            // Invalid ones look like: https://img.hellofresh.com/f_auto,fl_lossy,h_640,q_auto,w_1200/
            !url_part.ends_with('/')
        } else {
            true
        }
    });

    lines.join("\n")
}

// Regex to extract recipe image URL from HelloFresh page HTML
// Matches URLs like: https://media.hellofresh.com/w_1200,q_auto,f_auto,c_limit,fl_lossy/recipes/image/HF_xxx.jpg
static RECIPE_IMAGE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"https://(?:img|media)\.hellofresh\.com/w_(\d+)[^/\s"]+/recipes/image/([^"\s]+\.(?:jpg|jpeg|png|webp))"#).unwrap()
});

// Regex to extract PDF card link from HelloFresh page HTML
// Matches cardLink JSON field with PDF URL
static CARD_LINK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"cardLink"\s*:\s*"(https://[^"]+\.pdf)"#).unwrap()
});

/// Extract the PDF card link from the webpage HTML
fn extract_pdf_link(html: &str) -> Option<String> {
    CARD_LINK_RE.captures(html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract the main recipe image URL from the webpage HTML
fn extract_image_from_html(html: &str) -> Option<String> {
    // Find all recipe image URLs with their widths
    let mut images: Vec<(u32, String, String)> = Vec::new(); // (width, full_url, filename)

    for cap in RECIPE_IMAGE_RE.captures_iter(html) {
        if let (Some(width_str), Some(filename)) = (cap.get(1), cap.get(2)) {
            if let Ok(width) = width_str.as_str().parse::<u32>() {
                let full_url = cap.get(0).unwrap().as_str().to_string();
                let filename = filename.as_str().to_string();
                images.push((width, full_url, filename));
            }
        }
    }

    if images.is_empty() {
        return None;
    }

    // Prefer MAIN images with width around 1200
    let main_images: Vec<_> = images.iter()
        .filter(|(_, _, f)| f.contains("_Main") || f.contains("_MAIN") || f.contains("_main"))
        .collect();

    let target_images = if !main_images.is_empty() { main_images } else { images.iter().collect() };

    // Find image closest to 1200px width
    target_images.iter()
        .min_by_key(|(w, _, _)| (*w as i32 - 1200).abs())
        .map(|(_, url, _)| url.clone())
}

/// Add PDF URL to recipe content
fn add_pdf_to_content(content: &str, pdf_url: &str) -> String {
    // Insert PDF after the image line (or after title if no image)
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut insert_pos = None;

    // Find position after image, or after title if no image
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("image:") {
            insert_pos = Some(i + 1);
            break;
        }
        if line.starts_with("title:") && insert_pos.is_none() {
            insert_pos = Some(i + 1);
        }
    }

    if let Some(pos) = insert_pos {
        let pdf_line = format!("pdf: {}", pdf_url);
        lines.insert(pos, pdf_line);
        lines.join("\n")
    } else {
        content.to_string()
    }
}

/// Add image URL to recipe content if not already present
fn add_image_to_content(content: &str, image_url: &str) -> String {
    // Check if content already has a valid image
    for line in content.lines() {
        if line.starts_with("image:") && !line.trim().ends_with('/') {
            return content.to_string(); // Already has valid image
        }
    }

    // Insert image after the frontmatter header
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut insert_pos = None;

    // Find position after title in frontmatter
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("title:") {
            insert_pos = Some(i + 1);
            break;
        }
    }

    if let Some(pos) = insert_pos {
        let image_line = format!("image: {}", image_url);
        lines.insert(pos, image_line);
        lines.join("\n")
    } else {
        content.to_string()
    }
}

/// Download a single recipe using cooklang-import
/// Returns Ok(true) if downloaded, Ok(false) if skipped (no PDF), Err on failure
async fn download_recipe(
    client: &reqwest::Client,
    cooklang_import: &PathBuf,
    url: &str,
    output_path: &PathBuf,
) -> Result<bool> {
    debug!("Downloading: {}", url);

    // Fetch the page HTML to extract the PDF link and image
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch recipe page")?;

    if !response.status().is_success() {
        anyhow::bail!("Recipe page returned status: {}", response.status());
    }

    let html = response.text().await.context("Failed to read recipe page")?;

    // Extract PDF link - skip recipe if not found
    let pdf_url = match extract_pdf_link(&html) {
        Some(url) => url,
        None => {
            debug!("No PDF found for: {}", url);
            return Ok(false); // Skip this recipe
        }
    };

    // Extract image URL
    let image_url = extract_image_from_html(&html);

    // Run cooklang-import to get the recipe content
    let output = Command::new(cooklang_import)
        .arg(url)
        .arg("--download-only")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run cooklang-import")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cooklang-import failed: {}", stderr);
    }

    let content = String::from_utf8(output.stdout).context("Invalid UTF-8 in output")?;

    if content.trim().is_empty() {
        anyhow::bail!("Empty recipe content");
    }

    // Strip HTML tags and clean up content
    let content = strip_html_tags(&content);
    let content = clean_recipe_content(&content);

    // Add extracted image if available
    let content = if let Some(ref img_url) = image_url {
        add_image_to_content(&content, img_url)
    } else {
        content
    };

    // Add PDF link
    let content = add_pdf_to_content(&content, &pdf_url);

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::write(output_path, content)
        .await
        .context("Failed to write recipe file")?;

    Ok(true)
}

/// Process a single country
/// Returns (success, skipped_no_pdf, failed)
async fn process_country(
    client: &reqwest::Client,
    country: &Country,
    args: &Args,
) -> Result<(usize, usize, usize)> {
    let sitemap_url = country.sitemap_url();
    let country_dir = args.output.join(country.code);

    // Fetch sitemap
    let urls = match fetch_sitemap_urls(client, &sitemap_url).await {
        Ok(urls) => urls,
        Err(e) => {
            error!("Failed to fetch sitemap for {}: {}", country.name, e);
            return Ok((0, 0, 0));
        }
    };

    // Apply limit if specified
    let urls: Vec<_> = if let Some(limit) = args.limit {
        urls.into_iter().take(limit).collect()
    } else {
        urls
    };

    if urls.is_empty() {
        warn!("No recipes found for {}", country.name);
        return Ok((0, 0, 0));
    }

    // Create progress bar
    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_prefix(format!("{} ({})", country.name, country.code));

    let cooklang_import = args.cooklang_import.clone();
    let skip_existing = args.skip_existing;

    // Process URLs concurrently
    let results: Vec<_> = stream::iter(urls)
        .map(|url| {
            let client = client.clone();
            let cooklang_import = cooklang_import.clone();
            let country_dir = country_dir.clone();
            let pb = pb.clone();

            async move {
                let result = async {
                    let filename = url_to_filename(&url)
                        .ok_or_else(|| anyhow::anyhow!("Invalid URL format"))?;
                    let output_path = country_dir.join(&filename);

                    // Skip if exists
                    if skip_existing && output_path.exists() {
                        debug!("Skipping existing: {}", filename);
                        return Ok(Some(true)); // Existing counts as success
                    }

                    let downloaded = download_recipe(&client, &cooklang_import, &url, &output_path).await?;
                    Ok::<Option<bool>, anyhow::Error>(Some(downloaded))
                }
                .await;

                pb.inc(1);
                result
            }
        })
        .buffer_unordered(args.concurrency)
        .collect()
        .await;

    pb.finish();

    // Count results: Ok(Some(true)) = success, Ok(Some(false)) = skipped (no PDF), Err = failed
    let success = results.iter().filter(|r| matches!(r, Ok(Some(true)))).count();
    let skipped = results.iter().filter(|r| matches!(r, Ok(Some(false)))).count();
    let failed = results.iter().filter(|r| r.is_err()).count();

    // Log errors
    for (i, result) in results.iter().enumerate() {
        if let Err(e) = result {
            debug!("Recipe {} failed: {}", i, e);
        }
    }

    info!(
        "{}: {} downloaded, {} skipped (no PDF), {} failed",
        country.name, success, skipped, failed
    );

    Ok((success, skipped, failed))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("hellofresh_scraper=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();
    let countries = get_countries();

    // List countries mode
    if args.list_countries {
        println!("Available HelloFresh countries:");
        println!();
        for (code, country) in countries.iter() {
            println!("  {} - {} ({})", code, country.name, country.domain);
        }
        return Ok(());
    }

    // Verify cooklang-import exists
    if !args.cooklang_import.exists() {
        anyhow::bail!(
            "cooklang-import not found at: {}",
            args.cooklang_import.display()
        );
    }

    // Determine which countries to process
    let target_countries: Vec<_> = match &args.countries {
        Some(codes) => {
            let mut targets = Vec::new();
            for code in codes {
                match countries.get(code.as_str()) {
                    Some(country) => targets.push(country.clone()),
                    None => {
                        error!("Unknown country code: {}", code);
                        anyhow::bail!("Unknown country code: {}. Use --list-countries to see available codes.", code);
                    }
                }
            }
            targets
        }
        None => countries.values().cloned().collect(),
    };

    info!(
        "Processing {} countries: {:?}",
        target_countries.len(),
        target_countries.iter().map(|c| c.code).collect::<Vec<_>>()
    );

    // Create HTTP client
    let client = reqwest::Client::builder()
        .user_agent("HelloFresh-Recipe-Scraper/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Process each country
    let mut total_success = 0;
    let mut total_skipped = 0;
    let mut total_failed = 0;

    for country in target_countries {
        match process_country(&client, &country, &args).await {
            Ok((success, skipped, failed)) => {
                total_success += success;
                total_skipped += skipped;
                total_failed += failed;
            }
            Err(e) => {
                error!("Error processing {}: {}", country.name, e);
            }
        }
    }

    info!(
        "Complete! Total: {} downloaded, {} skipped (no PDF), {} failed",
        total_success, total_skipped, total_failed
    );

    Ok(())
}
