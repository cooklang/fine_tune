use std::{fmt::Write, io};
use anyhow::{Context, Result};
use cooklang::{
    convert::Converter,
    model::{Item, Section, Step},
    Recipe,
};

/// Simple options for markdown output
pub struct Options {
    pub heading: Headings,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            heading: Headings::default(),
        }
    }
}

pub struct Headings {
    pub ingredients: String,
    pub cookware: String,
    pub steps: String,
}

impl Default for Headings {
    fn default() -> Self {
        Self {
            ingredients: "Ingredients".into(),
            cookware: "Cookware".into(),
            steps: "Steps".into(),
        }
    }
}

pub fn print_md(
    recipe: &Recipe,
    name: &str,
    _scale: f64,
    converter: &Converter,
    mut writer: impl io::Write,
) -> Result<()> {
    let opts = Options::default();
    
    // Title
    writeln!(writer, "# {}", name).context("Failed to write title")?;
    writeln!(writer).context("Failed to write newline after title")?;
    
    // Metadata description as blockquote
    if let Some(desc) = recipe.metadata.description() {
        for line in desc.lines() {
            writeln!(writer, "> {}", line).context("Failed to write description")?;
        }
        writeln!(writer).context("Failed to write newline after description")?;
    }
    
    // Ingredients
    if !recipe.ingredients.is_empty() {
        writeln!(writer, "## {}", opts.heading.ingredients)
            .context("Failed to write ingredients header")?;
        writeln!(writer).context("Failed to write newline after ingredients header")?;
        
        for entry in recipe.group_ingredients(converter) {
            let ingredient = entry.ingredient;
            
            if !ingredient.modifiers().should_be_listed() {
                continue;
            }
            
            write!(writer, "- ").context("Failed to write ingredient bullet")?;
            if !entry.quantity.is_empty() {
                write!(writer, "{} ", entry.quantity)
                    .context("Failed to write quantity")?;
            }
            
            write!(writer, "{}", ingredient.display_name())
                .context("Failed to write ingredient name")?;
            
            if let Some(note) = &ingredient.note {
                write!(writer, " ({})", note).context("Failed to write ingredient note")?;
            }
            writeln!(writer).context("Failed to write newline after ingredient")?;
        }
        writeln!(writer).context("Failed to write newline after ingredients")?;
    }
    
    // Cookware
    if !recipe.cookware.is_empty() {
        writeln!(writer, "## {}", opts.heading.cookware)
            .context("Failed to write cookware header")?;
        writeln!(writer).context("Failed to write newline after cookware header")?;
        
        for item in recipe.group_cookware(converter) {
            let cw = item.cookware;
            write!(writer, "- ").context("Failed to write cookware bullet")?;
            if !item.quantity.is_empty() {
                write!(writer, "{} ", item.quantity).context("Failed to write amount")?;
            }
            write!(writer, "{}", cw.display_name()).context("Failed to write cookware name")?;
            
            if let Some(note) = &cw.note {
                write!(writer, " ({})", note).context("Failed to write cookware note")?;
            }
            writeln!(writer).context("Failed to write newline after cookware")?;
        }
        
        writeln!(writer).context("Failed to write newline after cookware list")?;
    }
    
    // Steps
    writeln!(writer, "## {}", opts.heading.steps).context("Failed to write steps header")?;
    writeln!(writer).context("Failed to write newline after steps header")?;
    
    for section in &recipe.sections {
        write_section(&mut writer, section, recipe)?;
    }
    
    Ok(())
}

fn write_section(
    w: &mut impl io::Write,
    section: &Section,
    recipe: &Recipe,
) -> Result<()> {
    // Section name (if present)
    if let Some(name) = &section.name {
        writeln!(w, "### {}", name).context("Failed to write section name")?;
        writeln!(w).context("Failed to write newline after section name")?;
    }
    
    for content in &section.content {
        match content {
            cooklang::Content::Step(step) => {
                write_step(w, step, recipe)?;
                writeln!(w).context("Failed to write newline after step")?;
                writeln!(w).context("Failed to write empty line after step")?;
            }
            cooklang::Content::Text(text) => {
                writeln!(w, "{}", text).context("Failed to write text content")?;
                writeln!(w).context("Failed to write newline after text")?;
            }
        }
    }
    Ok(())
}

fn write_step(w: &mut impl io::Write, step: &Step, recipe: &Recipe) -> Result<()> {
    let mut step_str = String::new();
    write!(&mut step_str, "{}. ", step.number).unwrap();

    for item in &step.items {
        match item {
            Item::Text { value } => step_str.push_str(value),
            &Item::Ingredient { index } => {
                let igr = &recipe.ingredients[index];
                step_str.push_str(igr.display_name().as_ref());
            }
            &Item::Cookware { index } => {
                let cw = &recipe.cookware[index];
                step_str.push_str(&cw.name);
            }
            &Item::Timer { index } => {
                let t = &recipe.timers[index];
                if let Some(quantity) = &t.quantity {
                    write!(&mut step_str, "{}", quantity).unwrap();
                }
            }
            &Item::InlineQuantity { index } => {
                let q = &recipe.inline_quantities[index];
                write!(&mut step_str, "{}", q).unwrap();
            }
        }
    }

    write!(w, "{}", step_str).context("Failed to write step")?;
    Ok(())
}

/// Render recipe as plain text in a reader-friendly format:
/// - Ingredients listed at the top (one per line, no bullets)
/// - Steps as paragraphs (no numbers)
pub fn print_plain(
    recipe: &Recipe,
    converter: &Converter,
    mut writer: impl io::Write,
) -> Result<()> {
    // Ingredients as plain lines
    for entry in recipe.group_ingredients(converter) {
        let ingredient = entry.ingredient;

        if !ingredient.modifiers().should_be_listed() {
            continue;
        }

        if !entry.quantity.is_empty() {
            write!(writer, "{} ", entry.quantity)
                .context("Failed to write quantity")?;
        }

        write!(writer, "{}", ingredient.display_name())
            .context("Failed to write ingredient name")?;

        if let Some(note) = &ingredient.note {
            write!(writer, " ({})", note).context("Failed to write ingredient note")?;
        }
        writeln!(writer).context("Failed to write newline after ingredient")?;
    }

    // Blank line between ingredients and steps
    writeln!(writer).context("Failed to write blank line")?;

    // Steps as paragraphs
    for section in &recipe.sections {
        write_section_plain(&mut writer, section, recipe)?;
    }

    Ok(())
}

fn write_section_plain(
    w: &mut impl io::Write,
    section: &Section,
    recipe: &Recipe,
) -> Result<()> {
    // Section name (if present)
    if let Some(name) = &section.name {
        writeln!(w, "=== {} ===", name).context("Failed to write section name")?;
        writeln!(w).context("Failed to write newline after section name")?;
    }

    for content in &section.content {
        match content {
            cooklang::Content::Step(step) => {
                write_step_plain(w, step, recipe)?;
                writeln!(w).context("Failed to write newline after step")?;
                writeln!(w).context("Failed to write empty line after step")?;
            }
            cooklang::Content::Text(text) => {
                writeln!(w, "{}", text).context("Failed to write text content")?;
                writeln!(w).context("Failed to write newline after text")?;
            }
        }
    }
    Ok(())
}

fn write_step_plain(w: &mut impl io::Write, step: &Step, recipe: &Recipe) -> Result<()> {
    let mut step_str = String::new();

    for item in &step.items {
        match item {
            Item::Text { value } => step_str.push_str(value),
            &Item::Ingredient { index } => {
                let igr = &recipe.ingredients[index];
                step_str.push_str(igr.display_name().as_ref());
            }
            &Item::Cookware { index } => {
                let cw = &recipe.cookware[index];
                step_str.push_str(&cw.name);
            }
            &Item::Timer { index } => {
                let t = &recipe.timers[index];
                if let Some(quantity) = &t.quantity {
                    write!(&mut step_str, "{}", quantity).unwrap();
                }
            }
            &Item::InlineQuantity { index } => {
                let q = &recipe.inline_quantities[index];
                write!(&mut step_str, "{}", q).unwrap();
            }
        }
    }

    write!(w, "{}", step_str).context("Failed to write step")?;
    Ok(())
}