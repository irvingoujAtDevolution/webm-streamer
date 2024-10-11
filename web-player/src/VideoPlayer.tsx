import { useRef, useEffect } from "react";
import "./App.css";

// VideoPlayer Component Props Interface
interface VideoPlayerProps {
	recordingToPlay: string;
}

const MIME_CODEC = 'video/webm; codecs="vp8"';
// VideoPlayer Component with Media Source API
export default function VideoPlayer({ recordingToPlay }: VideoPlayerProps) {
	const videoRef = useRef<HTMLVideoElement | null>(null);
	useEffect(() => {
		if (!videoRef.current || !recordingToPlay) return;

		const videoElement = videoRef.current;

		if (!window.MediaSource || !MediaSource.isTypeSupported(MIME_CODEC)) {
			console.error(
				"MediaSource API or the codec is not supported in this browser.",
			);
			return;
		}

		const mediaSource = new MediaSource();

		videoElement.src = URL.createObjectURL(mediaSource);

		mediaSource.addEventListener("sourceopen", () => {
			console.log("MediaSource opened");
			const fetchRecording = async () => {
				const response = await fetch(recordingToPlay, {
					method: "GET",
				});

				if (!response.ok) {
					console.error(`Failed to fetch video: ${response.statusText}`);
					return;
				}

				const recordingBlob = await response.blob();
				console.log(`blob size = ${recordingBlob.size}`);

				const sourceBuffer = mediaSource.addSourceBuffer(MIME_CODEC);
				const arrayBuffer = await recordingBlob.arrayBuffer();
				const uint8Array = new Uint8Array(arrayBuffer);

				// Ensure buffer is not updating before appending
				sourceBuffer.addEventListener("updateend", () => {
					console.log("SourceBuffer updateend");
					console.log(`state = ${mediaSource.readyState}`);
				});

				// Append buffer only when it's not updating
				sourceBuffer.appendBuffer(uint8Array);
			};

			fetchRecording();
		});

		return () => {
			// Clean-up: revoke the object URL when unmounting
		};
	}, [recordingToPlay]);

	return (
		<div className="video-container">
			<video ref={videoRef} width="640" height="360" controls>
				Your browser does not support the video tag.
			</video>
			{/* <video width="640" height="360" controls >
                <source src={recordingToPlay} type="video/webm" />
            </video> */}
		</div>
	);
}
