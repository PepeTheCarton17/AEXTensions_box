pub(crate) mod part1;
pub(crate) mod part2;
pub(crate) mod part3;
pub(crate) mod part4;
pub(crate) mod part5;
pub(crate) use part1::video_engine_part1;
pub(crate) use part2::video_engine_part2;
pub(crate) use part3::video_engine_part3;
pub(crate) use part4::video_engine_part4;
pub(crate) use part5::video_engine_part5;

macro_rules! video_engine_kt {
    () => {
        concat!(
        $crate::engine::generator::templates::video_engine::video_engine_part1!(),
        $crate::engine::generator::templates::video_engine::video_engine_part2!(),
        $crate::engine::generator::templates::video_engine::video_engine_part3!(),
        $crate::engine::generator::templates::video_engine::video_engine_part4!(),
        $crate::engine::generator::templates::video_engine::video_engine_part5!()
        )
    };
}
pub(crate) use video_engine_kt;
pub const VIDEO_ENGINE_KT: &str = video_engine_kt!();
