use std::{error::Error, fs::File, io::BufReader, path::Path};

pub fn read_from_file<P: AsRef<Path>, T: for<'de> serde::de::Deserialize<'de>>(
    path: P,
) -> Result<T, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read the JSON contents of the file as an instance of `User`.
    let u = serde_json::from_reader(reader)?;

    // Return the `User`.
    Ok(u)
}
