import { useRef, useEffect } from "react";
import "./App.css";
import { WebSocketWrapper } from "./streaming";

// VideoPlayer Component Props Interface
export interface VideoPlayerProps {
	recordingUrl: string;
}

const MIME_CODEC = 'video/webm; codecs="vp8"';
// VideoPlayer Component with Media Source API
export default function TestVideoPlayer({ recordingUrl: recordingToPlay }: VideoPlayerProps) {
	const videoRef = useRef<HTMLVideoElement | null>(null);
	const websocket = useRef<WebSocketWrapper | null>(null);
	useEffect(() => {
		if (!videoRef.current || !recordingToPlay) return;

		const videoElement = videoRef.current;

		if (!window.MediaSource || !MediaSource.isTypeSupported(MIME_CODEC)) {
			console.error(
				"MediaSource API or the codec is not supported in this browser.",
			);
			return;
		}

		const streaming = async () => {
			const mediaSource = new MediaSource();
			videoElement.src = URL.createObjectURL(mediaSource);

			websocket.current = new WebSocketWrapper(recordingToPlay);
			await new Promise((resolve) => {
				mediaSource.addEventListener("sourceopen", resolve);
			});
			await websocket.current.waitOpen();
			const sourceBuffer = mediaSource.addSourceBuffer(MIME_CODEC);
			const pullChunk = async () => {
				const response = await websocket.current?.send({
					Pull: { size: 1024 * 1024 },
				});
				if (response?.type === "Chunk") {
					sourceBuffer.appendBuffer(response.data);
				} else if (response?.type === "EOF") {
					console.log("Received EOF");
					mediaSource.endOfStream();
					return 'stop'
				}

				await new Promise((resolve) => {
					sourceBuffer.addEventListener("updateend", resolve);
				});
			};
			while (true) {
				console.log("Pulling chunk...");
				const result = await pullChunk();
				if (result === 'stop') {
					break;
				}
			}

			console.log("End of stream");
		};

		streaming();

		return () => {
			// Clean-up: revoke the object URL when unmounting
		};
	}, [recordingToPlay]);

	return (
		<div className="video-container">
			<video ref={videoRef} width="640" height="360" controls>
				Your browser does not support the video tag.
			</video>
		</div>
	);
}
