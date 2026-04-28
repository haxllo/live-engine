//! Scene loading and render-device abstractions.

mod device;
mod scene;

pub use device::{RenderDevice, RenderDeviceInfo, RenderError, create_shared_device};
pub use scene::{
    SceneConfig, SceneDescriptor, SceneFrameContext, ScenePipeline, SceneUniforms,
    build_scene_uniforms, load_scene_descriptor,
};
