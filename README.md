<p align="center">
    <img src="assets/light.png" width="70%"/>
</p>

This is a minimalistic blog platform for developers. It's based on markdown files and uses GitHub as a storage.

<p align="center">
    <b>
        <a href="https://uses.ink">Website</a>
        •
        <a href="https://cestef.uses.ink">Demo</a>
    </b>
</p>

## Self-hosting

You can easily self-host this platform with Docker. Start by copying the `.env.example` file to `.env` and fill in the required values.

> [!WARNING]
> Do NOT touch `.env.production` as it is used as a template for the production environment.

```bash
cp .env.example .env
```

> [!NOTE]
> The `REDIS_URL` value may seem a bit confusing (`redis://redis:6379`). This is because docker compose automatically creates a network for the services and uses the service name as the hostname. This is why we can use `redis` as the hostname.

Then, run the following command to start the platform:

```bash
docker compose --profile prod up -d
```

You can now access the platform at [`http://localhost:8765`](http://localhost:8765).

In case you want to use the latest commit from the repository, you can build the image locally with the `dev` profile:

```bash
docker compose --profile dev up -d
```

> [!IMPORTANT]
> When stopping the platform, make sure to stop the containers with the appropriate profile:
> ```bash
> docker compose --profile prod down # or --profile dev
> ```
> In case you're having trouble recreating the containers, use the `--force-recreate` flag.


## Development

This project uses [`pnpm`](https://pnpm.io) as the package manager. It is needed to properly install and patch the dependencies (See [Patches](#patches)).
To start the development server, run the following command:

```bash
# Install dependencies
pnpm install
# Start the development server
pnpm dev
```

## Patches

This project uses a patched version of [`hast-util-sanitize@5.0.1`](patches/hast-util-sanitize@5.0.1.patch) to allow unknown nodes to be processed through an `unknownNodeHandler` function. We need this to allow plugins such as [`remark-frontmatter`](https://github.com/remarkjs/remark-frontmatter) to pass declarations.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.


