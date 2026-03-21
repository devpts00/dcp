docker-clean:
	docker compose down -v --rmi all --remove-orphans

docker-pull:
	docker compose pull

docker-build:
	docker compose build rst

build-debug:
	docker compose run --rm --remove-orphans rst cargo build

build-release:
	docker compose run --rm --remove-orphans rst cargo build --release

run-debug: build-debug
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/debug/dcp \
		/dcp/usb/yakuza.mkv /dcp/dat/dst.dat --direct

run-release-donna: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
		--direct \
		--block-size=4KiB \
		--buffer-size=512KiB \
		--buffer-count=2 \
		/dcp/dat/donna.mkv /dcp/dat/donna.copy.mkv

run-release-yakuza: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
		--block-size=4KiB \
		--buffer-size=512KiB \
		--buffer-count=2 \
		/dcp/dat/yakuza.mkv /dcp/dat/yakuza.copy.mkv