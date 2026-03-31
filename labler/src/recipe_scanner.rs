use crate::RecipePair;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct RecipeScanner;

impl RecipeScanner {
    pub fn scan(root_path: &Path) -> Vec<RecipePair> {
        let mut recipe_map: HashMap<String, RecipeFiles> = HashMap::new();
        
        for entry in WalkDir::new(root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let extension = path.extension().and_then(|e| e.to_str());
            
            match extension {
                Some("recipe") | Some("cook") | Some("metadata") => {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let recipe_files = recipe_map.entry(stem.to_string()).or_insert(RecipeFiles {
                            markdown_path: None,
                            cooklang_path: None,
                            metadata_path: None,
                        });
                        
                        match extension {
                            Some("recipe") => recipe_files.markdown_path = Some(path.to_path_buf()),
                            Some("cook") => recipe_files.cooklang_path = Some(path.to_path_buf()),
                            Some("metadata") => recipe_files.metadata_path = Some(path.to_path_buf()),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        
        let mut recipes: Vec<RecipePair> = recipe_map
            .into_iter()
            .filter_map(|(name, files)| {
                if let (Some(markdown_path), Some(cooklang_path)) = 
                    (files.markdown_path, files.cooklang_path) {
                    Some(RecipePair {
                        id: 0,
                        name,
                        markdown_path,
                        cooklang_path,
                        metadata_path: files.metadata_path,
                    })
                } else {
                    None
                }
            })
            .collect();
        
        recipes.sort_by(|a, b| a.name.cmp(&b.name));
        
        for (index, recipe) in recipes.iter_mut().enumerate() {
            recipe.id = index;
        }
        
        recipes
    }
}

struct RecipeFiles {
    markdown_path: Option<PathBuf>,
    cooklang_path: Option<PathBuf>,
    metadata_path: Option<PathBuf>,
}
