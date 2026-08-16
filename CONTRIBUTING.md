# Contributing to rmod

First off, thank you for considering contributing to rmod! We appreciate your time and effort.

Whether you're helping us fix bugs, build new features, or improve our documentation, we'd love to have you on board.

And if you like the project, but just don't have time to contribute, that's fine. There are other easy ways to support the project and show your appreciation, which we would also be very happy about:

- Star the project
- Mention the project on social media
- Refer this project in your project's readme
- Mention the project at local meetups and tell your friends/colleagues

## How to Contribute

### 1. Find an Issue
You can start by looking through our open issues. If you want to work on something specific that isn't listed, please [create a new issue](https://github.com/ereinaimer/rmod/issues/new/choose) to discuss it before you begin writing code.

### 2. Fork and Branch
- Fork the repository and clone it locally.
    ```bash
    git clone https://github.com/ereinaimer/rmod.git
    cd rmod
    ```
- Create a new branch for your feature or bugfix: `feature/` or `fix/`
    ```bash
    git checkout -b feature/your-feature-name
    ```

### 3. Make Your Changes
- Write clear, concise code and include comments where necessary.
- Ensure your changes follow the existing coding style of the project.
- If you're adding a new feature, consider adding tests for it.

### 4. Test Your Code
Before submitting your changes, please make sure everything builds correctly and that all tests pass:

```bash
cargo check
cargo test
```

### 5. Optional Local Speedup
- **Windows: Dev Drive** — if you have Windows 11 22H2+, move the project and your Cargo registry (`~/.cargo`) to a Dev Drive. It bypasses Defender's real-time scan of the thousands of tiny files involved in a Rust build, which is often the single biggest local speedup on Windows.

### 6. Submit a Pull Request
- Create a Pull Request (PR) against our `main` branch.
- Use the provided PR template to describe your changes and link any relevant issues.
- Once submitted, we will review your PR and provide feedback!

## License

Please note that rmod is licensed under the MIT License (see [`LICENSE`](./LICENSE)). By contributing to the project, you agree to license your contributions under its terms.

## Community & Conduct

To ensure a welcoming environment for everyone, we ask that all contributors review and follow our [Code of Conduct](./CODE_OF_CONDUCT.md).

Thank you for helping make rmod better!