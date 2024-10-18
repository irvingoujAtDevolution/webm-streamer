import "./App.css";
import type { VideoPlayerProps } from "./TestVideoPlayer";

// VideoPlayer Component with Media Source API
export default function VideoPlayer({ recordingUrl: recordingToPlay }: VideoPlayerProps) {
	return (
		<div className="video-container">
			<video width="640" height="360" controls>
				<source src={recordingToPlay} type="video/webm" />
			</video>
		</div>
	);
}
