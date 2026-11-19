use noxl_layout::style::{SizeSpec, StyleError};

#[test]
fn resolves_auto_size() {
    let result = SizeSpec::Auto.resolve(400.0, 120.0);

    assert_eq!(result, Ok(120.0));
}

#[test]
fn resolves_pixel_size() {
    let result = SizeSpec::Px(150.0).resolve(400.0, 120.0);

    assert_eq!(result, Ok(150.0));
}

#[test]
fn resolves_percentage_size() {
    let result = SizeSpec::Percent(0.5).resolve(400.0, 120.0);

    assert_eq!(result, Ok(200.0));
}

#[test]
fn rejects_invalid_intrinsic_size() {
    let result = SizeSpec::Auto.resolve(400.0, f32::NAN);

    assert_eq!(result, Err(StyleError::InvalidIntrinsicSize));
}

#[test]
fn rejects_invalid_available_size() {
    let result = SizeSpec::Percent(0.5).resolve(-100.0, 120.0);

    assert_eq!(result, Err(StyleError::InvalidAvailableSize));
}

#[test]
fn rejects_percentage_arithmetic_overflow() {
    let result = SizeSpec::Percent(1.0).resolve(f32::MAX, 120.0);

    assert_eq!(result, Ok(f32::MAX));
}

#[test]
fn rejects_invalid_percentage() {
    let result = SizeSpec::Percent(1.5).resolve(400.0, 120.0);

    assert_eq!(result, Err(StyleError::InvalidPercentage));
}
