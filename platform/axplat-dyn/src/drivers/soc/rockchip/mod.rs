#[cfg(feature = "rockchip-pm")]
mod pm;

#[cfg(all(feature = "rk3588-clk", not(feature = "rk3568-clk")))]
#[path = "clk/rk3588-clk.rs"]
mod clk;

#[cfg(all(feature = "rk3568-clk", not(feature = "rk3588-clk")))]
#[path = "clk/rk3568-clk.rs"]
mod clk;
