/*
 * Starter Template for a new contract
 *
 * A contract consists of:
 * - [x] a module that must be exported top-level with the "borderless::contract" macro
 * - [x] a 'State', which must be defined inside the module
 * - [x] a bunch of 'action's that are defined as member-functions of the state
 *
 * Optional:
 * - [ ] a list of 'Sinks' that can be used to generate output events for other contracts or agents
 *
 */
#[borderless::contract]
pub mod __module_name__ {
    use borderless::prelude::*;

    // --- This is our state
    //
    // All fields must be serializable via serde.
    // The only exception are the datatypes in borderless::collections
    #[derive(State)]
    pub struct __StateName__ {
        switch: bool, //
    }

    // --- You can define Sinks to call other contracts or agents
    // use self::actions::Actions;
    // #[derive(NamedSink)]
    // pub enum Sinks {
    //     Flipper(Actions),
    // }

    impl __StateName__ {
        #[action]
        fn flip_switch(&mut self) {
            self.set_switch(!self.switch);
        }

        #[action(web_api = true, roles = "some-role")]
        fn set_switch(&mut self, switch: bool) {
            self.switch = switch;
        }
    }
}
