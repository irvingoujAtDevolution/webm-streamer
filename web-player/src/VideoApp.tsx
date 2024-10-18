import React, { useState, useEffect } from "react";
import "./App.css";
import VideoPlayer from "./VideoPlayer";
import TestVideoPlayer from "./TestVideoPlayer";
import StreamingVideoPlayer from "./StreamingVideoPlayer";
const HOST = "http://localhost:3000";
// const HOST = "https://work.tailf4f4d.ts.net";
const API_LIST_RECORDINGS = `${HOST}/jet/jrec/list-recording`;
const API_PULL_RECORDING = `${HOST}/jet/jrec/pull`;
const API_TEST_PULL_RECORDING = `${HOST}/jet/jrec/test`;
const API_STREAMING_PULL_RECORDING = `${HOST}/jet/jrec/stream-realtime`;
// const API_LIST_RECORDINGS = "http://localhost:3000/jet/jrec/list-recording";
// const API_PULL_RECORDING = "http://localhost:3000/jet/jrec/pull";
// const API_TEST_PULL_RECORDING = "http://localhost:3000/jet/jrec/test";

// Define the type for a recording tuple
type Recording = [filename: string, date: string];

// RecordingsList Component Props Interface
interface RecordingsListProps {
	recordings: Recording[];
	openRecordingInPopup: (recording: string, mode?: Mode) => void;
}

// RecordingsList Component
function RecordingsList({
	recordings,
	openRecordingInPopup,
}: RecordingsListProps) {
	return (
		<div className="recordings-list">
			<h2>Available Recordings</h2>
			<ul>
				{recordings.map(([filename, date]) => (
					<li key={filename}>
						{date} - {filename}
						<button onClick={() => openRecordingInPopup(filename, "play")}>
							Play
						</button>
						<button onClick={() => openRecordingInPopup(filename, "test")}>
							Play Test
						</button>
						<button onClick={() => openRecordingInPopup(filename, "stream")}>
							Play Stream
						</button>
					</li>
				))}
			</ul>
		</div>
	);
}

type Mode = "play" | "test" | "stream";

// Main VideoApp Component
function VideoApp() {
	const [recordings, setRecordings] = useState<Recording[]>([]);
	const [recordingToPlay, setRecordingToPlay] = useState<string | null>(null);
	const [currentMode, setCurrentMode] = useState<Mode>("play");

	const openRecordingInPopup = (recording: string, mode: Mode = "play") => {
		const popupWidth = 800;
		const popupHeight = 600;
		const left = (window.screen.width - popupWidth) / 2;
		const top = (window.screen.height - popupHeight) / 2;

		const urlParams = new URLSearchParams();
		urlParams.append("recording", recording);
		urlParams.append("mode", mode);

		const popupWindow = window.open(
			`${window.location.origin}${window.location.pathname}?${urlParams.toString()}`,
			"_blank",
			`width=${popupWidth},height=${popupHeight},top=${top},left=${left},resizable=yes`,
		);

		if (popupWindow) {
			popupWindow.focus();
		}
	};

	useEffect(() => {
		const fetchRecordings = async () => {
			const response = await fetch(API_LIST_RECORDINGS);
			const data: Recording[] = await response.json();
			setRecordings(data);
		};
		const urlParams = new URLSearchParams(window.location.search);
		const recording = urlParams.get("recording");
		const mode = urlParams.get("mode") as Mode;
		const api = (() => {
			if (mode === "stream") {
				return API_STREAMING_PULL_RECORDING;
			}
			if (mode === "test") {
				return API_TEST_PULL_RECORDING;
			}
			if (mode === "play") {
				return API_PULL_RECORDING;
			}
		})();
		if (recording) {
			setRecordingToPlay(`${api}?recording=${encodeURIComponent(recording)}`);
			setCurrentMode(mode);
		} else {
			fetchRecordings();
		}
	}, []);

	return (
		<div className="container">
			{(() => {
				if (recordingToPlay && currentMode === "play") {
					return <VideoPlayer recordingUrl={recordingToPlay} />;
				}

				if (recordingToPlay && currentMode === "test") {
					return <TestVideoPlayer recordingUrl={recordingToPlay} />;
				}

				if (recordingToPlay && currentMode === "stream") {
					return <StreamingVideoPlayer recordingUrl={recordingToPlay} />;
				}

				return (
					<RecordingsList
						recordings={recordings}
						openRecordingInPopup={openRecordingInPopup}
					/>
				);
			})()}
		</div>
	);
}

export default VideoApp;
