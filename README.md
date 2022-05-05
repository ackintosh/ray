# Ray

An Ethereum Beacon Node implementation _for learning opportunities for to-be-implementers_.

[![CI](https://github.com/ackintosh/ray/actions/workflows/ci.yml/badge.svg)](https://github.com/ackintosh/ray/actions/workflows/ci.yml)

![banner image](https://raw.githubusercontent.com/ackintosh/ray/898488c66bf520a5df71a8d28c562b12355af9ee/banner.jpeg)

> Photo by [Marc-Olivier Jodoin](https://unsplash.com/@marcojodoin?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText) on [Unsplash](https://unsplash.com/?utm_source=unsplash&utm_medium=referral&utm_content=creditCopyText)

## Overview

This project provides developers with an opportunity to learn how to implement Beacon Node, which is one of component of Ethereum Consensus Layer, via running the binary on testnet or reading the source codes. 

> NOTE: Ray is under active development.

We aim for a simpler implementation, by narrowing down the functions.

- Implemented for macOS
- Runs for Kiln testnet
- Simplicity over multifunctionality

### What does the Beacon Node do

- [x] [Node discovery](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#the-discovery-domain-discv5)
- RPC
  - [ ] [Status](https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status)
  - [ ] Goodbye
  - [ ] BeaconBlocksByRange
  - [ ] BeaconBlocksByRoot
  - [ ] Ping
  - [ ] GetMetaData
- [ ] Sync the Beacon Chain

## Getting started

### Running `ray`

```shell
$ git clone https://github.com/ackintosh/ray.git
$ cd ray
$ RUST_LOG=info cargo run
```

## Resources for the Consensus Layer implementers

Here are the specifications / documentations that Consensus Layer Implementers should refer to.

### Specs

- https://github.com/ethereum/consensus-specs
- https://github.com/ethereum/devp2p

### Documents

- https://github.com/timbeiko/eth-roadmap-faq
