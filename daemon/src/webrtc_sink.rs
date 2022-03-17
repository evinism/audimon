use anyhow::Result;
use bytes::Bytes;
use std::sync::Arc;
use tokio::time::Duration;
use webrtc::media::Sample;

use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::{TrackLocal};

pub async fn webrtc_sink(
    mut audio_buf_rx: tokio::sync::mpsc::Receiver<Vec<(i16, i16)>>,
    done_tx: tokio::sync::mpsc::Sender<()>,
) -> Result<(), anyhow::Error> {
    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    // Setup the codecs you want to use.
    m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                ..Default::default()
            },
            payload_type: 120,
            ..Default::default()
        },
        RTPCodecType::Audio,
    )?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);
    let audio_output_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            ..Default::default()
        },
        "track-audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    // Add this newly created track to the PeerConnection
    let rtp_sender = peer_connection
        .add_track(Arc::clone(&audio_output_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    let m = "audio".to_owned();
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        println!("{} rtp_sender.read loop exit", m);
        Result::<()>::Ok(())
    });

    // Wait for the offer to be pasted
    let line = signalz::must_read_stdin()?;
    let desc_data = signalz::decode(line.as_str())?;
    let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Set the remote SessionDescription
    peer_connection.set_remote_description(offer).await?;

    // Set a handler for when a new remote track starts, this handler copies inbound RTP packets,
    // replaces the SSRC and sends them back
    let _pc = Arc::downgrade(&peer_connection);

    // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
    // This is a temporary fix until we implement incoming RTCP events, then we would push a PLI only when a viewer requests it
    //let media_ssrc = track.ssrc();

    let output_track = Arc::clone(&audio_output_track);
    let output_track2 = Arc::clone(&output_track);

    tokio::spawn(async move {
        // Read RTP packets being sent to webrtc-rs
        let mut ticker = tokio::time::interval(Duration::from_millis(20));
        let mut encoder = audiopus::coder::Encoder::new(
            audiopus::SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::Audio,
        )
        .unwrap();
        encoder.set_complexity(8).unwrap();
        loop {
            let taken = audio_buf_rx.recv().await.unwrap();
            let mut in_buffer = [0i16; 960 * 2];
            //let mut in_buffer = [0i16; 960];
            for (i, sample) in taken.iter().enumerate() {
                //in_buffer[i] = (*sample).0;
                in_buffer[i * 2] = (*sample).0;
                in_buffer[i * 2 + 1] = (*sample).1;
            }
            let mut out_buffer = [0u8; 4096];
            let size = encoder.encode(&in_buffer, &mut out_buffer).unwrap();
            let data = Bytes::copy_from_slice(&out_buffer[0..size]);
            if let Err(err) = output_track2
                .write_sample(&Sample {
                    data: data,
                    duration: Duration::from_millis(20),
                    ..Default::default()
                })
                .await
            {
                println!("output track write_rtp got error: {}", err);
                break;
            }
            let _ = ticker.tick().await;
        }
    });

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection
        .on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            println!("Peer Connection State has changed: {}", s);

            if s == RTCPeerConnectionState::Failed {
                // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
                // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
                // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
                println!("Peer Connection has gone to failed exiting");
                let _ = done_tx.try_send(());
            }

            Box::pin(async {})
        }))
        .await;

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(answer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    // Output the answer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = signalz::encode(&json_str);
        println!("{}", b64);
    } else {
        println!("generate local_description failed!");
    }
    //let timeout = tokio::time::sleep(Duration::from_secs(20));
    //tokio::pin!(timeout);
    // TODO: return cleanup closure.
    // peer_connection.close().await?;
    return Ok(());
}
