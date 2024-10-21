use chrono::{DateTime, Local};
use clap::Parser;
use std::fs;
use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE};

#[derive(Parser)]
#[command(
    name = "size_monitor",
    about = "Monitors the size of files in a directory"
)]
struct AppArgs {
    /// Directory to monitor
    #[arg(short, long, default_value = "recordings")]
    directory: String,
}
fn main() -> anyhow::Result<()> {
    let args = AppArgs::parse();
    let recording_dir = PathBuf::from(args.directory);

    // Infinite loop to refresh the listing every 0.1 seconds
    loop {
        // Clear the console (this is platform-dependent; works on Windows)
        print!("\x1B[2J\x1B[1;1H");

        // Table header
        println!(
            "{:<30} {:<10} {:<20}",
            "File Name", "Size (bytes)", "Last Updated"
        );

        // Collect entries, sorting them by modification time
        let mut entries: Vec<_> = fs::read_dir(&recording_dir)?
            .filter_map(|entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let metadata = fs::metadata(&path).ok()?;
                    let modified_time = metadata.modified().ok()?;
                    Some((path, metadata, modified_time))
                } else {
                    None
                }
            })
            .collect();

        // Sort entries by the modified time in descending order (most recent first)
        entries.sort_by_key(|(_, _, modified_time)| *modified_time);
        entries.reverse();

        // Iterate over sorted entries and display them
        for (path, metadata, modified_time) in entries {
            // Get the file size
            let file_size = metadata.len();

            // Convert the last modified time to human-readable format
            let modified_time: DateTime<Local> = modified_time.into();
            let formatted_time = modified_time.format("%Y-%m-%d %H:%M:%S").to_string();

            // Get the file name
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => String::from("Unknown"),
            };

            // Open the file with read and write share modes
            let _file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_WRITE | FILE_SHARE_READ)
                .open(&path)?;

            // Print file details in table format
            println!("{:<30} {:<10} {:<20}", file_name, file_size, formatted_time);
        }

        // Sleep for 0.1 seconds before refreshing
        thread::sleep(Duration::from_millis(500));
    }
}
