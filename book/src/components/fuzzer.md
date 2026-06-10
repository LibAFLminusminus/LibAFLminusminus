# Fuzzer

`Fuzzer` is a the top-level module to describe what a fuzzer does in a fuzzing loop.
For example, our `StdFuzzer` is a `Fuzzer` implementation for mutational fuzzing will first pick a testcase chosen by the scheduler, then run the testcase with all the provided [`Stages`](./components/stages.md).
Just simple as that.

Of course you can think of other form `Fuzzer` implementation. 
A fuzzer for generational fuzzing will no longer need a corpus and a scheduler to pick testcase from. 
It can just generate testcase out of grammars and pass it to the stages.

Another important task for `Fuzzer` is to properly setup all the modules. 
This will include the dependency checking for the metadata used in modules and setting up signal handlers (necessary for inprocess fuzzing).
Usually you don't have to pay attention to these implementation details, but once you want create your own fuzzer loop, you should look at the implementation of `StdFuzzer`. 

## Fuzzer hooks

Fuzzer hooks is a collection of closures that users can set to call arbitrary routines at different context during a fuzzing loop. 
We implement this as `FuzzerHook` trait.

For now, we provide five insertion points. `pre_step`, `pre_add`, `post_add`, `pre_perform` and `post_step`. 
Each of these insertion point represents different events, a function to be called before executing one loop, before adding an input, after adding an input, before executing the stage, and right before the loop ends.
You can implement your customized hooks to add some functionalities when these event occurs.

# Component relationship

`Fuzzer` is a top-level module. 
Typically, `Fuzzer` contains a `FuzzerHook` object.