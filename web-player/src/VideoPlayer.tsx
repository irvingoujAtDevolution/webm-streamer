import "./App.css";

// VideoPlayer Component Props Interface
interface VideoPlayerProps {
	recordingToPlay: string;
}

// VideoPlayer Component with Media Source API
export default function VideoPlayer({ recordingToPlay }: VideoPlayerProps) {
	return (
		<div className="video-container">
			<video width="640" height="360" controls>
				<source src={recordingToPlay} type="video/webm" />
			</video>
		</div>
	);
}
