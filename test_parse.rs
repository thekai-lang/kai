use serde_yaml::Value;

fn main() {
    let content = std::fs::read_to_string("user.yaml").unwrap();
    let doc: Value = serde_yaml::from_str(&content).unwrap();
    let paths = doc.get("paths").unwrap().as_mapping().unwrap();
    for (k, v) in paths {
        println!("Path: {:?}", k.as_str());
    }
}
