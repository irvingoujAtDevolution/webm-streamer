const MediaRecorderRecordInterval = 10;
const recordingUrl = "ws://localhost:3000/jet/jrec/push";

export interface TerminalHandle {
	stop: () => void;
	onError(cb: () => void): void;
}

const openStreamingInPopup = (recording: string) => {
	const popupWidth = 800;
	const popupHeight = 600;
	const left = (window.screen.width - popupWidth) / 2;
	const top = (window.screen.height - popupHeight) / 2;

	const popupWindow = window.open(
		recording,
		"_blank",
		`width=${popupWidth},height=${popupHeight},top=${top},left=${left},resizable=yes`,
	);

	if (popupWindow) {
		popupWindow.focus();
	}

	return () => {
		if (popupWindow) {
			popupWindow.close();
		}
	};
};

export async function startRecorder(
	canvas: HTMLCanvasElement,
	openStreamer: boolean,
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
			// debugPrintHexDump(e);
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

	const closeWindow = await new Promise<() => void>((resolve) => {
		if (!openStreamer) {
			resolve(() => {});
			return;
		}
		ws.onmessage = (e) => {
			const blob = e.data as Blob;
			const stream = blob.stream();
			const reader = stream.getReader();
			reader.read().then((result) => {
				const filename = new TextDecoder().decode(result.value!);
				console.log("Recording filename:", filename);
				const url = `http://localhost:5174/?recording=${filename}&mode=stream`;
				const closeWindow = openStreamingInPopup(url);
				resolve(closeWindow);
			});
		};
	});

	return {
		stop: () => {
			mediaRecorder.stop();
			ws.close();
			closeWindow();
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
