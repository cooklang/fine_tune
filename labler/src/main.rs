use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

mod recipe_scanner;
mod cooklang_parser;
mod markdown_renderer;

use recipe_scanner::RecipeScanner;
use cooklang_parser::CooklangParser;

#[derive(Serialize, Deserialize)]
struct RecipePair {
    id: usize,
    name: String,
    markdown_path: PathBuf,
    cooklang_path: PathBuf,
    metadata_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct RecipeContent {
    id: usize,
    name: String,
    markdown_content: String,
    cooklang_content: String,
    metadata_content: Option<String>,
    parsed_result: ParsedRecipe,
    markdown_output: String,
    plain_output: String,
}

#[derive(Serialize, Deserialize)]
struct ParsedRecipe {
    ingredients: Vec<Ingredient>,
    cookware: Vec<String>,
    steps: Vec<String>,
    errors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Ingredient {
    name: String,
    quantity: Option<String>,
    unit: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct SaveRequest {
    content: String,
}

#[derive(Deserialize)]
struct SaveMarkdownRequest {
    content: String,
}

#[derive(Deserialize)]
struct ParseRequest {
    content: String,
}

fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            return content[3 + end + 4..].trim_start_matches('\n').to_string();
        }
    }
    content.to_string()
}

struct AppState {
    recipes: Vec<RecipePair>,
    recipe_root: PathBuf,
}

async fn get_recipes(data: web::Data<AppState>) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(&data.recipes))
}

async fn get_recipe(
    data: web::Data<AppState>,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    
    if id >= data.recipes.len() {
        return Ok(HttpResponse::NotFound().body("Recipe not found"));
    }
    
    let recipe = &data.recipes[id];
    
    let markdown_content = fs::read_to_string(&recipe.markdown_path)
        .unwrap_or_else(|_| String::from("Error reading markdown file"));
    let markdown_content = strip_frontmatter(&markdown_content);
    
    let cooklang_content = fs::read_to_string(&recipe.cooklang_path)
        .unwrap_or_else(|_| String::from("Error reading cooklang file"));
    
    let metadata_content = recipe.metadata_path.as_ref()
        .and_then(|path| fs::read_to_string(path).ok());
    
    let parsed_result = CooklangParser::parse(&cooklang_content);
    let markdown_output = CooklangParser::to_markdown_with_title(&cooklang_content, Some(&recipe.name));
    let plain_output = CooklangParser::to_plain(&cooklang_content);

    let content = RecipeContent {
        id,
        name: recipe.name.clone(),
        markdown_content,
        cooklang_content,
        metadata_content,
        parsed_result,
        markdown_output,
        plain_output,
    };
    
    Ok(HttpResponse::Ok().json(content))
}

async fn save_recipe(
    data: web::Data<AppState>,
    path: web::Path<usize>,
    req: web::Json<SaveRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    
    if id >= data.recipes.len() {
        return Ok(HttpResponse::NotFound().body("Recipe not found"));
    }
    
    let recipe = &data.recipes[id];
    
    fs::write(&recipe.cooklang_path, &req.content)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "saved"
    })))
}

async fn save_markdown(
    data: web::Data<AppState>,
    path: web::Path<usize>,
    req: web::Json<SaveMarkdownRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();
    
    if id >= data.recipes.len() {
        return Ok(HttpResponse::NotFound().body("Recipe not found"));
    }
    
    let recipe = &data.recipes[id];
    
    fs::write(&recipe.markdown_path, &req.content)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "saved"
    })))
}

async fn parse_cooklang(req: web::Json<ParseRequest>) -> Result<HttpResponse> {
    let parsed = CooklangParser::parse(&req.content);
    let markdown = CooklangParser::to_markdown(&req.content);
    let plain = CooklangParser::to_plain(&req.content);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "parsed": parsed,
        "markdown": markdown,
        "plain": plain
    })))
}

async fn delete_recipe(
    data: web::Data<AppState>,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let id = path.into_inner();

    if id >= data.recipes.len() {
        return Ok(HttpResponse::NotFound().body("Recipe not found"));
    }

    let recipe = &data.recipes[id];

    // Delete markdown/source file
    if let Err(e) = fs::remove_file(&recipe.markdown_path) {
        eprintln!("Failed to delete markdown file: {}", e);
    }

    // Delete cooklang file
    if let Err(e) = fs::remove_file(&recipe.cooklang_path) {
        eprintln!("Failed to delete cooklang file: {}", e);
    }

    // Delete metadata file if it exists
    if let Some(metadata_path) = &recipe.metadata_path {
        if let Err(e) = fs::remove_file(metadata_path) {
            eprintln!("Failed to delete metadata file: {}", e);
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "deleted",
        "next_index": if id < data.recipes.len() - 1 { id } else { id.saturating_sub(1) }
    })))
}

async fn index() -> Result<HttpResponse> {
    let html = include_str!("../static/index.html");
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args: Vec<String> = std::env::args().collect();
    let recipe_root = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("../recipes")
    };
    
    println!("Scanning recipes in: {:?}", recipe_root);
    let recipes = RecipeScanner::scan(&recipe_root);
    println!("Found {} recipe pairs", recipes.len());
    
    let app_state = web::Data::new(AppState {
        recipes,
        recipe_root,
    });
    
    println!("Starting server on http://localhost:8080");
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(index))
            .route("/api/recipes", web::get().to(get_recipes))
            .route("/api/recipe/{id}", web::get().to(get_recipe))
            .route("/api/recipe/{id}/cook", web::post().to(save_recipe))
            .route("/api/recipe/{id}/markdown", web::post().to(save_markdown))
            .route("/api/recipe/{id}", web::delete().to(delete_recipe))
            .route("/api/parse", web::post().to(parse_cooklang))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
