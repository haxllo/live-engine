use crate::app::LiveWallSettingsApp;

slint::include_modules!();

pub fn run_ui<C>(
    _app: LiveWallSettingsApp<C>,
) -> Result<(), Box<dyn std::error::Error>> 
where
    C: crate::app::ControlClient + Send,
{
    let window = SettingsWindow::new()?;
    window.run()?;
    Ok(())
}
