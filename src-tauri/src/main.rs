// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Évite l’ouverture d’une fenêtre de console sous Windows en version finale ; ne pas supprimer.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    privatetmpmondeensembletauri_lib::run()
}
