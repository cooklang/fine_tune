use crate::{Ingredient, ParsedRecipe};
use cooklang::{CooklangParser as Parser, Extensions, Converter};
use std::sync::Arc;

pub struct CooklangParser;

impl CooklangParser {
    pub fn parse(content: &str) -> ParsedRecipe {
        let mut ingredients = Vec::new();
        let mut cookware = Vec::new();
        let mut steps = Vec::new();
        let mut errors = Vec::new();
        
        let parser = Parser::new(Extensions::all(), Converter::default());
        let parse_result = parser.parse(content);
        
        // Get the recipe
        match parse_result.into_result() {
            Ok((mut recipe, warnings)) => {
                // Check for warnings
                if warnings.has_warnings() {
                    errors.push(format!("Warnings: {}", warnings));
                }
                // Scale to 1.0 to get the processed recipe
                recipe.scale(1.0, parser.converter());
                
                // Extract ingredients with proper grouping
                for entry in recipe.group_ingredients(parser.converter()) {
                    let ing = entry.ingredient;
                    let ingredient = Ingredient {
                        name: ing.display_name().to_string(),
                        quantity: if entry.quantity.is_empty() { 
                            None 
                        } else { 
                            Some(entry.quantity.to_string()) 
                        },
                        unit: None, // Unit is included in quantity string
                        note: ing.note.clone(),
                    };
                    ingredients.push(ingredient);
                }
                
                // Extract cookware
                for item in recipe.group_cookware(parser.converter()) {
                    let cw = item.cookware;
                    if !cookware.contains(&cw.name) {
                        cookware.push(cw.name.clone());
                    }
                }
                
                // Extract steps from sections
                for section in &recipe.sections {
                    // Add section name if present
                    if let Some(name) = &section.name {
                        steps.push(format!("=== {} ===", name));
                    }
                    
                    for content in &section.content {
                        match content {
                            cooklang::Content::Step(step) => {
                                let mut step_text = String::new();
                                for item in &step.items {
                                    match item {
                                        cooklang::model::Item::Text { value } => step_text.push_str(value),
                                        &cooklang::model::Item::Ingredient { index } => {
                                            let igr = &recipe.ingredients[index];
                                            step_text.push_str(igr.display_name().as_ref());
                                        }
                                        &cooklang::model::Item::Cookware { index } => {
                                            let cw = &recipe.cookware[index];
                                            step_text.push_str(&cw.name);
                                        }
                                        &cooklang::model::Item::Timer { index } => {
                                            let t = &recipe.timers[index];
                                            if let Some(quantity) = &t.quantity {
                                                step_text.push_str(&quantity.to_string());
                                            }
                                        }
                                        &cooklang::model::Item::InlineQuantity { index } => {
                                            let q = &recipe.inline_quantities[index];
                                            step_text.push_str(&q.to_string());
                                        }
                                    }
                                }
                                steps.push(step_text);
                            }
                            cooklang::Content::Text(text) => {
                                // Add text content (like blockquotes)
                                steps.push(text.clone());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Parse error: {}", e));
            }
        }
        
        ParsedRecipe {
            ingredients,
            cookware,
            steps,
            errors,
        }
    }
    
    pub fn to_markdown(content: &str) -> String {
        Self::to_markdown_with_title(content, None)
    }
    
    pub fn to_markdown_with_title(content: &str, title: Option<&str>) -> String {
        let parser = Parser::new(Extensions::all(), Converter::default());
        let parse_result = parser.parse(content);

        match parse_result.into_result() {
            Ok((mut recipe, _)) => {
                recipe.scale(1.0, parser.converter());
                let recipe = Arc::new(recipe);
                let title = title.unwrap_or("Recipe");

                // Use the markdown renderer
                let mut output = Vec::new();
                let _ = crate::markdown_renderer::print_md(
                    &recipe,
                    title,
                    1.0,
                    parser.converter(),
                    &mut output,
                );

                String::from_utf8_lossy(&output).to_string()
            }
            Err(e) => {
                format!("Error parsing recipe: {}", e)
            }
        }
    }

    /// Render recipe as plain text (ingredients at top, then step paragraphs)
    pub fn to_plain(content: &str) -> String {
        let parser = Parser::new(Extensions::all(), Converter::default());
        let parse_result = parser.parse(content);

        match parse_result.into_result() {
            Ok((mut recipe, _)) => {
                recipe.scale(1.0, parser.converter());
                let recipe = Arc::new(recipe);

                let mut output = Vec::new();
                let _ = crate::markdown_renderer::print_plain(
                    &recipe,
                    parser.converter(),
                    &mut output,
                );

                String::from_utf8_lossy(&output).to_string()
            }
            Err(e) => {
                format!("Error parsing recipe: {}", e)
            }
        }
    }
}