///
/// This is an independant implementation for reading and writing GUANO metadata.
/// The authors of this package are not associated with the authors of the reference implementation.
///
///
pub mod guano_file;

#[cfg(test)]
mod tests {

    use std::fs::File;

    use crate::guano_file::GuanoFile;

    use super::*;

    #[test]
    fn it_works() {
        let gf = GuanoFile::new(File::open("test").unwrap()).unwrap();
        assert_eq!(2 + 2, 4);
    }
}
