//! UI source supplies the layout. This tiny application supplies behavior/data.
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use terminal_ui_kit::Action;
use terminal_ui_language::Document;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = include_str!("../../../examples/ui-example.txt");
    let document = Document::parse(source)?;
    document.render(&BTreeMap::new())?.emit()?;
    for line in io::stdin().lock().lines() {
        let action: Action = serde_json::from_str(&line?)?;
        // Replace this branch with real application logic. No shell commands are inferred.
        if action.version == 1 && action.action == "greet" {
            document.render(&action.values)?.emit()?;
        }
    }
    Ok(())
}
