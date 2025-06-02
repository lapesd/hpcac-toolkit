use anyhow::{Result, bail};
use inquire::Confirm;
use tracing::error;

pub fn user_confirmation(skip_confirmation: bool, action_description: &str) -> Result<bool> {
    if !skip_confirmation {
        let confirm = Confirm::new(action_description)
            .with_default(false)
            .prompt();

        match confirm {
            Ok(true) => {
                println!("Confirmed! Proceeding...");
                Ok(true)
            }
            Ok(false) => {
                println!("Operation cancelled by user");
                Ok(false)
            }
            Err(e) => {
                error!("{}", e.to_string());
                bail!("Failure processing user response")
            }
        }
    } else {
        println!("Automatic confirmation with -y flag. Proceeding...");
        Ok(true)
    }
}
