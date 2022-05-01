# Ray

An Ethereum Consensus Layer implementation for learning opportunities for to-be-implementers.

[![CI](https://github.com/ackintosh/ray/actions/workflows/ci.yml/badge.svg)](https://github.com/ackintosh/ray/actions/workflows/ci.yml)

This project is not intended for mainnet, provides developers with an opportunity to learn how to implement the Consensus Layer via running the binary on testnet or reading the source codes. 

We aim for a simpler implementation, by narrowing down the functions.

- Implemented for macOS
- Runs for Kiln testnet
- Simplicity over multifunctionality

## Specs

Here are the specifications that Consensus Layer Implementors should refer to.

- https://github.com/ethereum/consensus-specs
- https://github.com/ethereum/devp2p

## Getting started

```shell
$ git clone https://github.com/ackintosh/ray.git
$ cd ray
$ RUST_LOG=info cargo run
```
