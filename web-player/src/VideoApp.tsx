import React, { useRef, useState, useEffect } from "react";
import "./App.css";

const API_LIST_RECORDINGS = "http://localhost:3000/jet/jrec/list-recording";
const API_PULL_RECORDING = "http://localhost:3000/jet/jrec/pull";
const API_TEST_PULL_RECORDING = "http://localhost:3000/jet/jrec/test";

function VideoApp() {
	const videoRef = useRef<HTMLVideoElement | null>(null);
	const [recordings, setRecordings] = useState<string[][]>([]);
	const [recordingToPlay, setRecordingToPlay] = useState<string | null>(null);

	const fetchRecordings = async () => {
		const response = await fetch(API_LIST_RECORDINGS);
		const data = await response.json();
		setRecordings(data);
	};

	const openRecordingInPopup = (recording: string, test = false) => {
		const popupWidth = 800;
		const popupHeight = 600;
		const left = (window.screen.width - popupWidth) / 2;
		const top = (window.screen.height - popupHeight) / 2;

		const popupWindow = window.open(
			`${window.location.origin}${window.location.pathname}?recording=${recording}&test=${test}`,
			"_blank",
			`width=${popupWidth},height=${popupHeight},top=${top},left=${left},resizable=yes`,
		);

		if (popupWindow) {
			popupWindow.focus();
		}
	};

	useEffect(() => {
		const urlParams = new URLSearchParams(window.location.search);
		const recording = urlParams.get("recording");
		const test = urlParams.get("test");
		const api = test === 'true' ? API_TEST_PULL_RECORDING : API_PULL_RECORDING;
		if (recording) {
			setRecordingToPlay(`${api}?recording=${recording}`);
		} else {
			fetchRecordings();
		}
	}, []);

	return (
		<div className="container">
			{recordingToPlay ? (
				<div className="video-container">
					<video ref={videoRef} width="640" height="360" controls autoPlay>
						<source src={recordingToPlay} type="video/webm" />
						Your browser does not support the video tag.
					</video>
				</div>
			) : (
				<div className="recordings-list">
					<h2>Available Recordings</h2>
					<ul>
						{recordings.map(([filename, date]) => (
							<li key={filename}>
								{date} - {filename}
								<button onClick={() => openRecordingInPopup(filename)}>
									Play
								</button>
								<button onClick={() => openRecordingInPopup(filename, true)}>
									Play Test
								</button>
							</li>
						))}
					</ul>
				</div>
			)}
		</div>
	);
}

export default VideoApp;
