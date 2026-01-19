# Split Screen

## Prototype

The way I made the prototype is by hardcoding the two different textures for two different screen. It works. And I just wanted to make sure it works before moving to a better way of handling it. Because I always wanted to make the number of parallel simulations something that the user can choose and not something hardcoded.

In the prototype I added the textures as following:

```rust
// texture names for referencing
pub const TEX_RD1_PING: &str = "rd1 ping";
pub const TEX_RD1_PONG: &str = "rd1 pong";
pub const TEX_RD1_TEMP: &str = "rd1 temp";
pub const TEX_RD1_OUTPUT: &str = "rd1 output";

pub const TEX_RD2_PING: &str = "rd2 ping";
pub const TEX_RD2_PONG: &str = "rd2 pong";
pub const TEX_RD2_TEMP: &str = "rd2 temp";
pub const TEX_RD2_OUTPUT: &str = "rd2 output";
```

I don't like it... but it works.
So I thought of another way.

## Slots

There are arrays of slots, `slot[0]`, `slot[1]`, ... and each slot may or may not host a simulation node. Split screen is just the answer to the question "How many slots are active?".

So something like

```rust
pub struct SimSlotConfig {
    pub slot_idx: u32,
    pub enabled: bool,
}
```

I will do it in 2 phases because I want to make sure that it works for 2 simulations first... and IF it worked I move to a more dynamic method. I already have an idea, The number of chosen simulation generates the texture names. so instead of `TEX_RD1_PING` etc. I go back to `"rd1 ping"` or even better `"rd1_ping"` but more like `"rd{}_ping"` and `{}` is where the index of the simulation is written in.
