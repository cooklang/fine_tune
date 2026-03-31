# Cooklang Recipe Validation Tool - Requirements

## Overview
A Rust-powered web application for validating and correcting conversions from classical recipe format (markdown) to Cooklang format. The tool will display recipe pairs side-by-side with live editing capabilities and parsing validation.

## Core Requirements

### Technology Stack
- **Backend**: Rust web server (using Actix-Web or Rocket)
- **Parser**: Official Cooklang Rust parser (`cooklang` crate)
- **Frontend**: HTML/CSS/JavaScript with live editing
- **Storage**: File-based (reading from filesystem)

### Data Structure
- Recipe files located in: `/Users/alexeydubovskoy/cooklang/cooklang-ml/recipe-archive/recipes`
- Nested folder structure supported
- File types per recipe:
  - `.md` - Original recipe in markdown format
  - `.cook` - Converted Cooklang format
  - `.metadata` - Optional metadata file

### Functional Requirements

#### 1. Recipe Loading & Navigation
- Load all recipe pairs from the specified directory recursively
- Support nested folder structures
- Match recipes by base filename (e.g., "Chocolate Chip Cookies.md" with "Chocolate Chip Cookies.cook")
- Navigate between recipe pairs using:
  - Cmd+Left Arrow: Previous recipe
  - Cmd+Right Arrow: Next recipe
- Display current recipe index (e.g., "Recipe 3 of 150")

#### 2. Display Layout (Single Screen)
- **Left Panel**: Original recipe (.md file)
  - Read-only display
  - Rendered markdown or raw text
  - Scrollable
  
- **Middle Panel**: Cooklang Editor
  - Editable text area
  - No Syntax highlighting
  - Auto-save capability
  - Save indicator
  
- **Right Panel**: Parsed Output
  - **Ingredients Section**:
    - Listed with quantities and units
    - Grouped if applicable
  - **Method Section**:
    - Steps with highlighted ingredients and cookware
    - Timer displays
  - **Parsing Errors** (if any):
    - Clear error messages
    - Line numbers where errors occur

#### 3. Editing Features
- Live editing of Cooklang source
- Real-time parsing and preview update (with debouncing)
- Save changes back to .cook file
- Undo/Redo support
- Dirty state indicator

#### 4. Validation Features
- Parse Cooklang using official Rust parser
- Display parsing errors prominently
- Highlight problematic lines in editor
- Show validation status (valid/invalid)

#### 5. Keyboard Shortcuts
- Cmd+S: Save current .cook file
- Cmd+Left: Previous recipe
- Cmd+Right: Next recipe
- Cmd+Z: Undo
- Cmd+Shift+Z: Redo

### Technical Requirements

#### Backend API Endpoints
- `GET /api/recipes` - List all recipe pairs
- `GET /api/recipe/{id}` - Get specific recipe pair data
- `POST /api/recipe/{id}/cook` - Save updated Cooklang content
- `POST /api/parse` - Parse Cooklang and return structured data

#### Parser Integration
- Use official `cooklang` crate from crates.io
- Handle parsing errors gracefully
- Extract:
  - Ingredients with quantities
  - Equipment/cookware
  - Steps with annotations
  - Timers
  - Metadata

#### Performance
- Lazy loading of recipes (don't load all at once)
- Debounced parsing (e.g., 500ms after typing stops)
- Caching of parsed results
- Fast navigation between recipes

### UI/UX Requirements

#### Visual Design
- Clean, minimal interface
- Clear visual separation between panels
- Responsive layout (minimum 1280px width)
- Dark/light mode support (optional)

#### Status Indicators
- Current recipe name in header
- Save status (saved/unsaved/saving)
- Parse status (valid/invalid/parsing)
- Navigation position (e.g., "45/200")

#### Error Handling
- Clear error messages for parsing failures
- Network error handling
- File access error handling
- Graceful degradation

### Data Persistence
- Changes saved directly to .cook files
- Optional: Change history/versioning
- Optional: Export corrections for ML training

### Development Phases

#### Phase 1: MVP
- Basic file loading and navigation
- Display original and Cooklang side-by-side
- Edit and save Cooklang
- Basic parsing and display

#### Phase 2: Enhanced Editing
- Syntax highlighting
- Real-time parsing
- Better error display
- Keyboard shortcuts

#### Phase 3: Advanced Features
- Search functionality
- Bulk operations
- Export for ML training
- Statistics dashboard

## Dependencies

### Rust Crates (Preliminary)
```toml
[dependencies]
actix-web = "4.4"
cooklang = "0.14"  # Official parser
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
walkdir = "2"
pulldown-cmark = "0.9"  # For markdown parsing
```

### Frontend Libraries
- No heavy frameworks required (vanilla JS preferred)
- Optional: CodeMirror or Monaco for syntax highlighting
- Optional: Marked.js for markdown rendering

## Testing Requirements
- Unit tests for parser integration
- Integration tests for API endpoints
- Manual testing checklist for UI interactions
- Test with sample recipe dataset

## Deployment
- Single binary executable
- Configuration via environment variables or config file
- Docker container (optional)
- Default port: 8080

## Success Criteria
- Successfully load and navigate through all recipe pairs
- Edit and save Cooklang files without data loss
- Parse and display Cooklang correctly using official parser
- Smooth navigation with keyboard shortcuts
- Clear indication of parsing errors
- Fast enough for reviewing thousands of recipes
