use mac_address::get_mac_address;
use wifi_rs::{prelude::*, WiFi};


// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
   log::info!("Tauri is awesome!");
   match get_mac_address() {
           Ok(Some(ma)) => {
               format!("MAC, {}! You've been greeted from Rust!", ma)
           }
           Ok(None) => format!("No mac {}! !", name),
           Err(_) => format!("err, {}! You've been greeted from Rust!", name), //println!("{:?}", e),
       }
}


#[tauri::command]
fn greet2(name: &str) -> String {
   log::info!("Tauri is awesome2!");

   let config = Some(Config {
           interface: Some("en0"),
       });

   let mut wifi = WiFi::new(config);

   match wifi.connect("Device", "gimmeinternet") {
       Ok(result) => println!(
           "{}",
           if result == true {
               "Connection Successful."
           } else {
               "Invalid password."
           }
       ),
       Err(err) => println!("The following error occurred: {:?}", err),
   }

   format!("YES, {}! You've been greeted from Rust!", name)

}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}



