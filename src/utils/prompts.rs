use anyhow::Result;
use inquire::Confirm;

pub fn user_confirmation(skip_confirmation: bool, action_description: &str) -> Result<bool> {
    if !skip_confirmation {
        let confirm = Confirm::new(action_description)
            .with_default(false)
            .prompt();

        match confirm {
            Ok(true) => {
                tracing::info!("Confirmed! Proceeding...");
                Ok(true)
            }
            Ok(false) => {
                tracing::info!("Operation cancelled by user");
                Ok(false)
            }
            Err(e) => {
                tracing::error!("{}", e.to_string());
                anyhow::bail!("Failure processing user response")
            }
        }
    } else {
        tracing::info!("Automatic confirmation with -y flag. Proceeding...");
        Ok(true)
    }
}
