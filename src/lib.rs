//! # csc411_rpegio
//!
//! A collection functions to handle rpeg data i/o. Intended for use in URI's CSC 411 class.

use std::iter::Peekable;

fn expect(
    expected_bytes: &[u8],
    peekable_bytes_iter: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<(), String> {
    for expected_byte in expected_bytes {
        match &peekable_bytes_iter.next() {
            Some(byte) => {
                if byte != expected_byte {
                    return Err(format!(
                        "Expected 0x{expected_byte:02X}, found 0x{byte:02X}"
                    ));
                }
            }
            None => {
                return Err(format!(
                    "Ran out of bytes before expected 0x{expected_byte:02X} byte"
                ));
            }
        }
    }
    Ok(())
}

fn expect_newline(
    peekable_bytes_iter: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<(), String> {
    match peekable_bytes_iter.next() {
        // \n - Mostly Unix
        Some(0x0A) => Ok(()),
        // \r[\n] - Mostly Windows
        Some(0x0D) => {
            // Check for a \n after the \r, consuming it if it exists
            if peekable_bytes_iter.peek() == Some(&0x0A) {
                peekable_bytes_iter.next();
            }

            Ok(())
        }
        Some(byte) => Err(format!("Expected newline byte(s), found 0x{byte:02X}")),
        None => Err("Ran out of bytes before expected newline byte(s)".to_string()),
    }
}

fn is_ascii_digit(byte: u8) -> bool {
    (b'0'..=b'9').contains(&byte)
}

fn parse_ascii_digit(digit: u8) -> Result<u32, String> {
    if !is_ascii_digit(digit) {
        Err(format!("Attempted to parse non-ascii digit {digit:?}"))
    } else {
        Ok((digit - b'0') as u32)
    }
}

fn read_u32(peekable_bytes_iter: &mut Peekable<impl Iterator<Item = u8>>) -> Result<u32, String> {
    // Read initial digit (there ought to be at least one)
    let mut next_byte = match peekable_bytes_iter.peek() {
        Some(&byte) => byte,
        None => return Err("Didn't find a number where a number was expected in input".to_string()),
    };

    let mut num = parse_ascii_digit(next_byte)?;
    peekable_bytes_iter.next();

    // Read any additional digits in the number
    while peekable_bytes_iter.peek().is_some()
        && is_ascii_digit(*peekable_bytes_iter.peek().unwrap())
    {
        next_byte = peekable_bytes_iter.next().unwrap();
        let digit = parse_ascii_digit(next_byte).unwrap();

        num = num
            .checked_mul(10)
            .and_then(|num| num.checked_add(digit))
            .ok_or("Integer overflow while parsing u32".to_string())?;
    }

    Ok(num)
}

fn read_raw_bytes(file_path: Option<&str>) -> Result<Vec<u8>, std::io::Error> {
    let mut raw_reader: Box<dyn std::io::BufRead> = match file_path {
        Some(file_path) => Box::new(std::io::BufReader::new(std::fs::File::open(file_path)?)),
        None => Box::new(std::io::BufReader::new(std::io::stdin())),
    };

    // read the entire contents into a buffer
    let mut buffer = Vec::new();
    raw_reader.read_to_end(&mut buffer)?;

    Ok(buffer)
}

/// Reads and parses rpeg data from either stdin or a file.
/// Returns a Result<tuple, String> where the tuple contains, in order:
/// 1. A `Vec<[u8; 4]>` (Vector of four-byte arrays) representing the raw image data
/// 2. A `u32` representing the width of the image
/// 3. A `u32` representing the height of the image
///
/// # Errors Returned
///
/// * If there is an unexpected error reading from the provided file or stdin
/// * If the rpeg data header is badly formatted
/// * If the number of raw bytes following the header is not a multiple of 4
///
/// # Arguments
///
/// * `file_path` - An optional file path to read from. If None, stdin will be read from instead
///
/// # Examples
/// ```no_run
/// // Read rpeg data from stdin to variables for later use
/// let (raw_bytes, width, height) = csc411_rpegio::read_in_rpeg_data(Some("path/to/file.ppm")).unwrap();
///
/// // Do something with width and height. This is just an example
/// println!("Image size: {width}x{height}");
///
/// // Do something with raw_bytes
/// // you will likely first want to convert the four-byte arrays to u32s
/// ```
pub fn read_in_rpeg_data(file_path: Option<&str>) -> Result<(Vec<[u8; 4]>, u32, u32), String> {
    // Read stdin as bytes
    let bytes = read_raw_bytes(file_path)
        .map_err(|_| "Error reading raw bytes from the input".to_string())?;
    let mut peekable_bytes_iter = bytes.into_iter().peekable();

    // Read "Compressed image format 2\n" part of header
    expect(b"Compressed image format 2", &mut peekable_bytes_iter)?;
    expect_newline(&mut peekable_bytes_iter)?;

    // Read "{width} {height}\n" part of header
    let width = read_u32(&mut peekable_bytes_iter)?;
    expect(b" ", &mut peekable_bytes_iter)?;
    let height = read_u32(&mut peekable_bytes_iter)?;
    expect_newline(&mut peekable_bytes_iter)?;

    // Collect the rest of the bytes (after the header) as a vector of u8s
    let raw_bytes: Vec<u8> = peekable_bytes_iter.collect();

    // Group the bytes in groups of 4
    if raw_bytes.len() % 4 != 0 {
        return Err(format!(
            "The number of raw bytes ({}) was not a multiple of four",
            raw_bytes.len()
        ));
    }

    let grouped_bytes: Vec<[u8; 4]> = raw_bytes
        .chunks_exact(4)
        .map(|x| x.try_into().unwrap())
        .collect();

    Ok((grouped_bytes, width, height))
}

/// Outputs rpeg data to stdout.
///
/// # Arguments
///
/// * `raw_bytes` - A slice of four-byte arrays, each array representing a single word of
///    compressed image data
/// * `width` - The width of the image
/// * `height` - The height of the image
///
/// # Panics
///
/// * If something goes wrong writing raw bytes to stdout
///
/// # Examples
/// ```
/// // In your program, this rpeg data would be generated by compressing a .ppm file.
/// // Here, I've just made up some random data
/// let width: u32 = 2;
/// let height: u32 = 1;
/// let raw_bytes: Vec<[u8; 4]> = vec![[0x00, 0x11, 0x22, 0x33], [0x44, 0x55, 0x66, 0x77]];
///
/// // Output the rpeg data to stdout
/// csc411_rpegio::output_rpeg_data(&raw_bytes, width, height);
/// ```
pub fn output_rpeg_data(raw_bytes: &[[u8; 4]], width: u32, height: u32) {
    use std::io::Write;

    println!("Compressed image format 2");
    println!("{width} {height}");

    for bytes in raw_bytes {
        std::io::stdout()
            .write_all(bytes)
            .expect("Failed to write raw bytes to stdout");
    }
}

/// Outputs rpeg data to stdout in a human-readable form. This should NOT be used outside of
/// debugging and testing.
///
/// # Arguments
///
/// * `raw_bytes` - A slice of four-byte arrays, each array representing a single word of
///    compressed image data
/// * `width` - The width of the image
/// * `height` - The height of the image
///
/// # Examples
/// ```
/// // In your program, this rpeg data would be generated by compressing a .ppm file.
/// // Here, I've just made up some random data
/// let width: u32 = 2;
/// let height: u32 = 1;
/// let raw_bytes: Vec<[u8; 4]> = vec![[0x00, 0x11, 0x22, 0x33], [0x44, 0x55, 0x66, 0x77]];
///
/// // Output the rpeg data to stdout
/// csc411_rpegio::debug_output_rpeg_data(&raw_bytes, width, height);
///
/// // Standard Output:
/// // Compressed image format 2 [DEBUG]
/// // 2 1
/// // 00 11 22 33 44 55 66 77
/// ```
pub fn debug_output_rpeg_data(raw_bytes: &[[u8; 4]], width: u32, height: u32) {
    println!("Compressed image format 2 [DEBUG]");
    println!("{width} {height}");

    let mut first = true;

    for bytes in raw_bytes {
        for byte in bytes {
            if first {
                first = false;
            } else {
                print!(" ");
            }

            print!("{byte:02X}");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_no_tests() {
        panic!("I don't know how to test this because it is very dependent on exact i/o to stdio");
    }
}
