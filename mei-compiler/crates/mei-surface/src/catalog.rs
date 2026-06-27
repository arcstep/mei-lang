use crate::builtins::{surface_descriptors, SurfaceDescriptor};

pub fn surface_catalog() -> Vec<SurfaceDescriptor> {
    surface_descriptors()
}
