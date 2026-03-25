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

run-release-donna-io-uring: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
		io-uring \
		--direct \
		--buffer-size=16MiB \
		--buffer-count=2 \
		--src=/dcp/dat/donna.mkv \
		--dst=/dcp/dat/donna.copy.mkv

run-release-yakuza-io-uring: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
		io-uring \
		--direct \
		--buffer-size=16MiB \
		--buffer-count=2 \
		--src=/dcp/dat/yakuza.mkv \
		--dst=/dcp/dat/yakuza.copy.mkv

run-release-donna-stream: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
	  	stream \
	  	--direct \
		--buffer-size=16MiB \
		--src=/dcp/usb/donna.mkv \
		--dst=/dcp/dat/donna.copy.mkv

run-release-yakuza-stream: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
	  	stream \
	  	--direct \
		--buffer-size=16MiB \
		--src=/dcp/usb/yakuza.mkv \
		--dst=/dcp/dat/yakuza.copy.mkv

run-release-donna-syscall: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
	  	syscall \
		--src=/dcp/dat/donna.mkv \
		--dst=/dcp/dat/donna.copy.mkv

run-release-yakuza-syscall: build-release
	docker compose run --rm -it --remove-orphans \
		--name dcp rst ./target/release/dcp \
	  	syscall \
	  	--chunk-size=16MiB \
		--src=/dcp/dat/yakuza.mkv \
		--dst=/dcp/dat/yakuza.copy.mkv
