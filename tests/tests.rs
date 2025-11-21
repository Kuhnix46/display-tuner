use display_tuner::display;

#[test]
fn test_display_info() {
    let displays = display::enumerate_displays().unwrap();
    assert!(!displays.is_empty());
    
    let first = &displays[0];
    assert_ne!(first.friendly_name, "");
    assert!(first.width > 0);
    assert!(first.height > 0);
    assert!(first.scaling_current > 0);
    assert!(first.scaling_recommended > 0);
    
    println!("{first}");
}