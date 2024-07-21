# 🖋️ uses.ink 

This is a minimalistic blog platform for developers. It's based on markdown files and uses GitHub as a storage.

**Head over to the [website](https://uses.ink) to get started.**

## Self-hosting

You can easily self-host this platform with Docker. Start by copying the `.env.example` file to `.env` and fill in the required values.

```bash
cp .env.example .env
```

> [!NOTE]
> The `REDIS_URL` value may seem a bit confusing (`redis://redis:6379`). This is because docker compose automatically creates a network for the services and uses the service name as the hostname. This is why we can use `redis` as the hostname.

Then, run the following command to start the platform:

```bash
docker compose up -d
```

You can now access the platform at `http://localhost:3000`.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.


