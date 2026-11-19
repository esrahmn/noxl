use noxl_core::geometry::Size;
use noxl_core::layout::{
    Alignment, Constraints, LayoutConfig, LayoutDirection, LayoutError, Padding,
};

fn main() {
    let config = LayoutConfig::new(
        LayoutDirection::Horizontal,
        10.0,
        Padding::all(10.0),
        Alignment::Center,
    );

    let container = Size {
        width: 400.0,
        height: 120.0,
    };

    let sizes = [
        Size {
            width: 50.0,
            height: 20.0,
        },
        Size {
            width: 150.0,
            height: 150.0,
        },
    ];

    let constraints = [
        Constraints::new(100.0, 200.0, 40.0, 80.0).unwrap(),
        Constraints::new(100.0, 200.0, 40.0, 80.0).unwrap(),
    ];

    // Pengujian berhasil
    let result = config.calculate_layout_with_constraints(container, &sizes, &constraints);

    println!("Valid layout: {:#?}", result);

    // Pengujian jumlah Constraints tidak cocok
    let invalid_result =
        config.calculate_layout_with_constraints(container, &sizes, &constraints[..1]);

    println!("Invalid layout: {:#?}", invalid_result);

    assert_eq!(invalid_result, Err(LayoutError::ConstraintCountMismatch));

    println!("Error handling test passed!");
}
