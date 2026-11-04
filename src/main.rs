mod config;

fn main() {
    let _settings = config::Settings::load(std::path::Path::new("settings.json"));
}
