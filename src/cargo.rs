use cmd::call;
use error::FatalError;
use Features;

pub fn publish(dry_run: bool, features: Features) -> Result<bool, FatalError> {
    match features {
        Features::None => call(vec![env!("CARGO"), "publish"], dry_run),
        Features::Selective(vec) => call(
            vec![env!("CARGO"), "publish", "--features", &vec.join(" ")],
            dry_run,
        ),
        Features::All => call(vec![env!("CARGO"), "publish", "--all-features"], dry_run),
    }
}

pub fn update(dry_run: bool) -> Result<bool, FatalError> {
    call(vec![env!("CARGO"), "update"], dry_run)
}

pub fn doc(dry_run: bool) -> Result<bool, FatalError> {
    call(vec![env!("CARGO"), "doc", "--no-deps"], dry_run)
}
