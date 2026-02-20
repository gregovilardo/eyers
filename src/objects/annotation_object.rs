use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

use crate::services::annotations::Annotation;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct AnnotationObject {
        pub annotation: RefCell<Annotation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AnnotationObject {
        const NAME: &'static str = "AnnotationObject";
        type Type = super::AnnotationObject;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for AnnotationObject {}
}

glib::wrapper! {
    pub struct AnnotationObject(ObjectSubclass<imp::AnnotationObject>);
}

impl AnnotationObject {
    pub fn new(annotation: Annotation) -> Self {
        let obj: Self = glib::Object::builder().build();

        obj.imp().annotation.replace(annotation);

        obj
    }

    pub fn annotation(&self) -> Annotation {
        self.imp().annotation.borrow().clone()
    }
}
