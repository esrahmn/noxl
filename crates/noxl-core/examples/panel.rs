use noxl_core::color::Color;
use noxl_core::component::{Component, ComponentType};
use noxl_core::tree::ComponentNode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let panel = Component::builder(ComponentType::Panel)
        .size(1600.0, 40.0)
        .background(Color::BLACK)
        .build()?;

    let mut root = ComponentNode::new(panel);

    let music_player = Component::builder(ComponentType::Container)
        .size(1600.0, 40.0)
        .background(Color::BLACK)
        .build()?;

    let music_id = music_player.id;

    root.add_child(ComponentNode::new(music_player))
        .expect("Failed to add music player");

    if let Some(node) = root.find_mut(music_id) {
        node.component.props.background = Color::RED;
        println!("Music Player color updated!");
    }

    if let Some(node) = root.find(music_id) {
        println!("New color: {:?}", node.component.props.background);
    }

    if let Some(removed) = root.remove(music_id) {
        println!("Component removed: {:?}", removed.component.id);
    }

    println!("Still exists: {}", root.contains(music_id));
    println!("{:#?}", root);

    Ok(())
}
