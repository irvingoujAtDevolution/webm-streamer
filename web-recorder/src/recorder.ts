const MediaRecorderRecordInterval = 10;
const recordingUrl = "ws://localhost:3000/jet/jrec/push";

export interface TerminalHandle {
	stop: () => void;
	onError(cb: () => void): void;
}

export async function startRecorder(
	canvas: HTMLCanvasElement,
): Promise<TerminalHandle | undefined> {
	if (!canvas) {
		console.error("IronRDP canvas not found");
		return;
	}

	let onErrorCb = () => {};

	const stream = canvas.captureStream();
	const mediaRecorder = new MediaRecorder(stream, { mimeType: "video/webm" });

	const ws = new WebSocket(recordingUrl);
	ws.onopen = () => {
		mediaRecorder.start(MediaRecorderRecordInterval);
		mediaRecorder.ondataavailable = (e) => {
			// console log the data in hex dump, for debugging
			debugPrintHexDump(e);
			ws.send(e.data);
		};
		mediaRecorder.onstop = () => {
			if (ws.readyState === ws.OPEN) {
				ws.close();
			}
		};
	};

	ws.onerror = (e) => {
		console.error("Error in recording websocket", e);
		mediaRecorder.stop();
		onErrorCb();
	};

	ws.onclose = () => {
		mediaRecorder.stop();
		console.log("Recording websocket closed");
	};

	return {
		stop: () => {
			mediaRecorder.stop();
			ws.close();
		},
		onError: (cb: () => void) => {
			onErrorCb = cb;
		},
	};
}
function debugPrintHexDump(e: BlobEvent) {
  e.data.arrayBuffer().then((buffer) => {
    const view = new DataView(buffer);
    let hex = "";
    for (let i = 0; i < view.byteLength; i++) {
      hex += view.getUint8(i).toString(16).padStart(2, "0") + " ";
    }
    console.log(hex);
  });
}

