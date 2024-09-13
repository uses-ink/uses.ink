<p align="center">
    <img src="assets/light.png#gh-light-mode-only" width="70%"/>
    <img src="assets/dark.png#gh-dark-mode-only" width="70%"/>
</p>

This is a minimalistic blog platform for developers. It's based on markdown files and uses GitHub as a storage.

<p align="center">
    <b>
        <a href="https://uses.ink">Website</a>
        •
        <a href="https://cestef.uses.ink">Demo</a>
    </b>
</p>

> [!NOTE]
> A rewrite of this project with [`astro`](https://astro.build) is in progress. The legacy version will be kept in the [`legacy`](https://github.com/uses-ink/uses.ink/tree/legacy) branch.

## Self-hosting

### Quick and easy  

You can copy the [`docker-compose.yml`](docker-compose.yml) file and use the [Docker image](https://github.com/uses-ink/uses.ink/pkgs/container/uses.ink) to quickly spin up a self-hosted instance of the platform.

```bash
curl -o docker-compose.yml https://raw.githubusercontent.com/uses-ink/uses.ink/main/docker-compose.yml
docker compose up -d
```

### I want to do it myself

You can either build the Docker image yourself (why would you do that?) or run the platform directly on your machine, see the [Development](#development) and [Building](#building) sections.

## Development

This project uses [`bun`](https://bun.sh) as the package manager. It is needed to properly install and patch the dependencies (See [Patches](#patches)).

Make sure to have a `redis` server running somewhere. You can use the following command to quicly spin up a redis server with docker:

```bash
docker run -d --name redis -p 6379:6379 redis
```

Populate the `apps/web/.env` file with the required values:

```bash
cp apps/web/.env.example apps/web/.env
```

> [!NOTE]
> You don't need to set a `GITHUB_TOKEN` for development. The platform will work without it.

To start the development server, run the following commands:

```bash
# Install dependencies
bun install
# Start the development server
bun --cwd apps/web dev
```

## Building

<details>
<summary>Docker</summary>

To build the Docker image, run the following command:

```bash
docker build -f docker/Dockerfile -t uses.ink .
```
</details>
<details>
<summary>Local</summary>

To build the project, run the following command:

```bash
bun build
```
</details>


## Miscellaneous dependencies

### Patches
This project uses a patched version of [`hast-util-sanitize@5.0.1`](patches/hast-util-sanitize@5.0.1.patch) to allow unknown nodes to be processed through an `unknownNodeHandler` function. We need this to allow plugins such as [`remark-frontmatter`](https://github.com/remarkjs/remark-frontmatter) to pass declarations.

### d2wasm

> [!NOTE]
> This section is currently not relevant because the platform uses the `d2` CLI due to the poor performance of the wasm module. Documentation is kept for future reference. 

This project uses [`d2`](https://d2lang.com) to render diagrams. The diagrams are rendered using WebAssembly on the server. The module comes from the [`d2wasm`](https://github.com/uses-ink/d2wasm) repository. It can be found in the [`wasm`](wasm/) directory.

To update the wasm module, run the following command:

```bash
pnpm update-d2wasm
```

This will grab the latest version of the wasm module and place it in its appropriate directory.

## Contributing

Contributions are welcome! Feel free to open an [issue](https://github.com/uses-ink/uses.ink/issues) or a [pull request](https://github.com/uses-ink/uses.ink/pulls).

I am currently looking for help with the following:

- [ ] Improve the [`d2wasm`](https://github.com/uses-ink/d2wasm) WebAssembly module performance / size.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.


