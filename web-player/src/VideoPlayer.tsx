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
			let total_bytes_received = 0;
			let total_bytes_expected = 0;
			const fetchRecording = async () => {
				const response = await fetch(recordingToPlay, {
					method: "GET",
                    mode: "cors",
					headers: {
						Range: `bytes=${total_bytes_received}-`,
					},
				});

				if (!response.ok) {
					console.error(`Failed to fetch video: ${response.statusText}`);
					return;
				}

				const recordingBlob = await response.blob();
				const sourceBuffer = mediaSource.addSourceBuffer(MIME_CODEC);
				const arrayBuffer = await recordingBlob.arrayBuffer();

				if (response.status === 200) {
					sourceBuffer.appendBuffer(arrayBuffer);
				} else if (response.status === 206) {
					const contentRange = response.headers.get("content-range");
					if (!contentRange) {
						console.error("Content-Range header is missing");
						return;
					}
					const { start, end, total } = ParseContentRange(contentRange);
					sourceBuffer.onupdateend = () => {
						console.log("sourceBuffer.onupdateend");
						total_bytes_received = total_bytes_received + (end - start + 1);
						total_bytes_expected = total;
						if (total_bytes_received < total_bytes_expected) {
							fetchSubsequentRecording(sourceBuffer);
						}
					};

					sourceBuffer.appendBuffer(arrayBuffer);
				}
			};
            const timer = () => new Promise((resolve) => setTimeout(resolve, 50));
            let count = 0;
            const awaitForEvery = async (n: number ) => {
                if (count % n === 0) { await timer(); }
                count++;
            }
			const fetchSubsequentRecording = async (sourceBuffer: SourceBuffer) => {
                await awaitForEvery(30);
				const response = await fetch(`${recordingToPlay}&count=${count}`, {
					method: "GET",
					headers: {
						Range: `bytes=${total_bytes_received}-`,
					},
				});

				if (!response.ok) {
					console.error(`Failed to fetch video: ${response.statusText}`);
					return;
				}

				const recordingBlob = await response.blob();
				const arrayBuffer = await recordingBlob.arrayBuffer();

				if (response.status === 206) {
					const contentRange = response.headers.get("content-range");
					if (!contentRange) {
						console.error("Content-Range header is missing");
						return;
					}
					const { start, end, total } = ParseContentRange(contentRange);

					// Update the total bytes received and expected
					total_bytes_received += end - start + 1;
					total_bytes_expected = total;

					sourceBuffer.appendBuffer(arrayBuffer);

					sourceBuffer.onupdateend = () => {
                        console.log(`sourceBuffer.onupdateend: ${total_bytes_received} / ${total_bytes_expected}`);
						if (total_bytes_received < total_bytes_expected) {
							fetchSubsequentRecording(sourceBuffer);
						} else {
							mediaSource.endOfStream();
						}
					};
				} else {
					console.error(`Unexpected response status: ${response.status}`);
				}
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

function ParseContentRange(contentRange: string): {
	start: number;
	end: number;
	total: number;
} {
	const [, range] = contentRange.split(" ");
	const [start_end, total] = range.split("/");
	const [start, end] = start_end.split("-");
	return {
		start: Number.parseInt(start),
		end: Number.parseInt(end),
		total: Number.parseInt(total),
	};
}
