export interface ClientRequest {
	Pull?: {
		size?: number;
	};
}

export interface Metadata {
	chunk_size: number;
	offset: number;
	total_size: number;
}

export type ServerResponse =
	| { type: "Chunk"; metadata: Metadata ; data: Uint8Array }
	| { type: "EOF" };

export function tryToServerResponse(data: Uint8Array): ServerResponse | null {
	if (data.length < 1) {
		return null; // Invalid data
	}

	const typeCode = data[0];
	let offset = 1;

	if (typeCode === 0x01) {
		// Handle EOF case (assuming type code 0x00 represents EOF)
		return { type: "EOF" };
	}
	if (typeCode === 0x00) {
		// Handle Chunk case (assuming type code 0x01 represents Chunk)

		// Read the next 4 bytes for the metadata length
		if (data.length < offset + 4) {
			return null; // Invalid data, not enough bytes for metadata length
		}
		const metadataLength = new DataView(data.buffer).getUint32(offset, false);
		offset += 4;

		let metadata: Metadata | null = null;
		if (metadataLength > 0) {
			if (data.length < offset + metadataLength) {
				return null; // Invalid data, not enough bytes for metadata
			}
			const metadataBytes = data.slice(offset, offset + metadataLength);
			offset += metadataLength;

			// Parse the metadata as JSON
			try {
				const metadataJson = new TextDecoder().decode(metadataBytes);
				metadata = JSON.parse(metadataJson) as Metadata;
			} catch (e) {
				console.error("Failed to parse metadata:", e);
				return null;
			}
		}

        if (!metadata) {
            return null; // Invalid data, metadata is required
        }

		// The rest of the data is the actual payload
		const chunkData = data.slice(offset);
		return { type: "Chunk", metadata, data: chunkData };
	}

	return null; // Unknown type code
}

export class WebSocketWrapper {
	private websocket: WebSocket;

	constructor(url: string) {
		this.websocket = new WebSocket(url);
	}

	onOpen(callback: (event: Event) => void) {
		this.websocket.onopen = callback;
	}

	async waitOpen() {
		return new Promise<void>((resolve) => {
			if (this.websocket.readyState === WebSocket.OPEN) {
				resolve();
			} else {
				this.onOpen(() => resolve());
			}
		});
	}

	async send(data: ClientRequest): Promise<ServerResponse> {
		this.websocket.send(JSON.stringify(data));

		return new Promise<ServerResponse>((resolve, reject) => {
			this.websocket.onmessage = (event) => {
				const blob = event.data as Blob;
				const stream = blob.stream();

				stream
					.getReader()
					.read()
					.then((result) => {
						const value = result.value!;
						const serverResponse = tryToServerResponse(value);
						if (!serverResponse) {
							reject("Invalid server response");
							return;
						}
						resolve(serverResponse);
					});
			};

			this.websocket.onerror = (event) => {
				reject(event);
			};
		});
	}
}
