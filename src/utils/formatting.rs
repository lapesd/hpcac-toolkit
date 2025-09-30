pub fn format_processor_info(core_count: Option<i64>, cpu_architecture: String, cpu_type: String) -> String {
    match &core_count {
        Some(cores) => {
            format!(
                "{}-Core {} {}",
                cores, cpu_architecture, cpu_type
            )
        }
        None => {
            format!(
                "{} {}", cpu_architecture, cpu_type
            )
        }
    }
}