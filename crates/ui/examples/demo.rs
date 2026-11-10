use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use terminal_ui_kit::{Action, View};

fn screen(name: &str) -> View {
    View::new("hello", "UI framework demo")
        .text("Sample application / real two-way events")
        .heading(format!("Hello, {name}"))
        .input("name", "Your name", name)
        .action("greet", "Update greeting")
        .table(
            vec!["Component".into(), "State".into()],
            vec![vec!["UI protocol".into(), "Connected".into()]],
        )
        .progress("Example progress", 0.65)
        .code(
            "powershell",
            "Get-ChildItem -Path $HOME | Select-Object Name",
        )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    screen("world").emit()?;
    for line in io::stdin().lock().lines() {
        let event: Action = serde_json::from_str(&line?)?;
        if event.version == 1 && event.view == "hello" && event.action == "greet" {
            let values: BTreeMap<_, _> = event.values;
            screen(values.get("name").map(String::as_str).unwrap_or("world")).emit()?;
        }
    }
    Ok(())
}
