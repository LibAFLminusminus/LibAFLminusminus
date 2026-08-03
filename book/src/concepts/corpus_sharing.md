# Corpus Sharing

Corpus sharing is a more advanced topic.
It can take multiple forms, and using the right kind of corpus sharing reveals to be subtle and often wrong.

It is recommended to take a look at the [`Controller & Worker`](../components/controller_worker.md)'s documentation first, as this page often refers to these components.

## Main Idea

Corpus sharing is anything related to sharing test cases between workers.
Even through most fuzzers tend to propose a single way to share a corpus, strategies can in fact be extremely diverse, depending on multiple factors: worker configuration, time between synchronizations, order of synchronized inputs, etc...

In our definition, corpus sharing is not limited to "sharing inputs between workers to increase coverage".
Instead, we mean by there any form of input exchange between workers.
Think of a scenario in which you run some fuzzing workers, and would like to send all the newly found inputs in another client that runs a sanitizer to check for memory corruption bugs.
We consider this a form of corpus sharing.

## Disclaimer

Latest academic work does not conclude whether corpus sharing is always beneficial to the overall coverage.
In other words, running `N` fuzzers in parallel without any input shared tends to be better than well-known corpus sharing strategies (like the AFL++ `1 to N` topology).
It does not necessarily mean corpus sharing is useless, but it also means it's not trivial to conclude it will be useful if you goal is to maximize coverage.
It is also highly likely the effectiveness of corpus sharing will vary depending on the target, the fuzzer configuration, etc...

This is the main reason why we let the possibility to fully configure corpus sharing in `LibAFL--`.
We do not know what will work better for you, so you are responsible for finding the corpus sharing settings that work best for you.

# Synchronization Architecture

To synchronize inputs (and information in general) between workers, we use the composition of multiple traits.

# Standard Corpus Sharing

As explained above, it is currently unclear is corpus sharing is even useful, or in which configuration it would actually work.
Thus, we decided to keep as the standard orchestrator a non-sharing orchestrator: no input get ever sent over between workers.
We will keep things that way until new work can properly show corpus sharing can improve coverage, and in which condition.
