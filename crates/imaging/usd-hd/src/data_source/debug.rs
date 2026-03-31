
//! Debug print for data sources.
//!
//! Port of HdDebugPrintDataSource from pxr/imaging/hd/dataSource.h/cpp

use super::base::HdDataSourceBaseHandle;
use super::container::cast_to_container;
use super::vector::cast_to_vector;
use std::fmt::Write;

/// Print a data source to a stream for debugging/testing.
///
/// Recursively prints the structure: containers show child names,
/// vectors show indices, sampled sources show their value at t=0.
///
/// Port of HdDebugPrintDataSource(std::ostream&, HdDataSourceBaseHandle, int).
pub fn hd_debug_print_data_source(
    out: &mut impl Write,
    data_source: Option<&HdDataSourceBaseHandle>,
    indent_level: usize,
) -> std::result::Result<(), std::fmt::Error> {
    let indent: String = "\t".repeat(indent_level);

    let Some(ds) = data_source else {
        writeln!(out, "{indent}NULL")?;
        return Ok(());
    };

    if let Some(container) = cast_to_container(ds) {
        let mut names = container.get_names();
        names.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        for name in names {
            if let Some(child) = container.get(&name) {
                writeln!(out, "{indent}[{}]", name.as_str())?;
                hd_debug_print_data_source(out, Some(&child), indent_level + 1)?;
            }
        }
    } else if let Some(vector) = cast_to_vector(ds) {
        let n = vector.get_num_elements();
        for i in 0..n {
            if let Some(elem) = vector.get_element(i) {
                writeln!(out, "{indent}[{i}]")?;
                hd_debug_print_data_source(out, Some(&elem), indent_level + 1)?;
            }
        }
    } else if let Some(sampled) = ds.as_sampled() {
        let value = sampled.get_value(0.0);
        writeln!(out, "{indent}{value}")?;
    } else {
        writeln!(out, "{indent}UNKNOWN")?;
    }

    Ok(())
}

/// Print a data source to stdout for debugging/testing.
///
/// Port of HdDebugPrintDataSource(HdDataSourceBaseHandle, int).
pub fn hd_debug_print_data_source_stdout(
    data_source: Option<&HdDataSourceBaseHandle>,
    indent_level: usize,
) {
    let mut s = String::new();
    if hd_debug_print_data_source(&mut s, data_source, indent_level).is_ok() {
        print!("{s}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::retained::HdRetainedSampledDataSource;
    use super::*;
    use usd_vt::Value;

    #[test]
    fn test_debug_print_sampled() {
        let ds: HdDataSourceBaseHandle = HdRetainedSampledDataSource::new(Value::from(42i32));
        let mut s = String::new();
        hd_debug_print_data_source(&mut s, Some(&ds), 0).unwrap();
        assert!(s.contains("42"));
    }
}
