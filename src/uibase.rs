use std::cell::RefCell;
use std::rc::Rc;

use crate::platform::{Point, Rect};
use crate::raster::Image;

pub type UINodeRef = Rc<RefCell<Box<dyn UIBase>>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Margin {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Padding {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub trait UIBase {
    fn id(&self) -> u32;
    fn draw(&self, rect: Rect, image: &mut Image) -> bool;
    fn checkhit(&self, point: Point) -> Option<u32>;
    fn get_rect(&self) -> Rect;
    fn update_rect(&mut self, parent: &Rect, padding: &Padding);
    fn children(&self) -> &[UINodeRef] {
        &[]
    }
}

#[derive(Clone, Debug)]
pub struct UINodeBase {
    pub id: u32,
    pub rect: Rect,
    pub margin: Margin,
    pub padding: Padding,
    pub visible: bool,
}

impl UINodeBase {
    pub fn new(id: u32, rect: Rect) -> Self {
        Self {
            id,
            rect,
            margin: Margin::default(),
            padding: Padding::default(),
            visible: true,
        }
    }
}

pub struct UIRoot {
    root: UINodeRef,
}

impl UIRoot {
    pub fn new(root: UINodeRef) -> Self {
        Self { root }
    }

    pub fn root(&self) -> UINodeRef {
        Rc::clone(&self.root)
    }

    pub fn findbyid(&self, id: u32) -> Option<UINodeRef> {
        find_node(&self.root, id)
    }

    pub fn itor_byorder(&self) -> Vec<UINodeRef> {
        let mut out = Vec::new();
        collect_nodes(&self.root, &mut out);
        out
    }

    pub fn redraw_rect(&self, id: u32) -> Option<Rect> {
        self.findbyid(id).map(|node| node.borrow().get_rect())
    }
}

fn find_node(node: &UINodeRef, id: u32) -> Option<UINodeRef> {
    if node.borrow().id() == id {
        return Some(Rc::clone(node));
    }
    for child in node.borrow().children() {
        if let Some(found) = find_node(child, id) {
            return Some(found);
        }
    }
    None
}

fn collect_nodes(node: &UINodeRef, out: &mut Vec<UINodeRef>) {
    out.push(Rc::clone(node));
    for child in node.borrow().children() {
        collect_nodes(child, out);
    }
}
