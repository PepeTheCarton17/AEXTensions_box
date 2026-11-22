pub(crate) mod part1;
pub(crate) mod part2;
pub(crate) mod part3;
pub(crate) use part1::anime_source_kt_part1;
pub(crate) use part2::anime_source_kt_part2;
pub(crate) use part3::anime_source_kt_part3;

macro_rules! anime_source_kt_fmt {
    () => {
        concat!(
        $crate::engine::generator::templates::anime_source::anime_source_kt_part1!(),
        $crate::engine::generator::templates::anime_source::anime_source_kt_part2!(),
        $crate::engine::generator::templates::anime_source::anime_source_kt_part3!()
        )
    };
}
pub(crate) use anime_source_kt_fmt;
