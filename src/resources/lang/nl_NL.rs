#[macro_export]
macro_rules! t {
    ("Yes") => ("Ja");
    ("No") => ("Nee");
    ("Mode") => ("Modus");
    ("Control") => ("Bediening");
    ("Text editor") => ("Tekstverwerker");
    ("Speech") => ("Spraak");
    ("Error") => ("Fout");
    ("I/O error") => ("I/O fout");
    ("Arduino response error") => ("Arduino-antwoordfout");
    ("Arduino verification error") => ("Arduino-verificatiefout");
    ("Request '{}' failed with error: {}") => ("Aanvraag '{}' mislukte met de foutmelding: {}");
    ("Verification failed{}") => ("Verificatie mislukt{}");
    ("Error while drawing") => ("Fout tijdens het tekenen");
    ("Could not initialize the COM library") => ("Kon de COM-bibliotheek niet initialiseren");
    ("Could not enumerate the available tokens") => ("Kon de beschikbare tokens niet opvragen");
    ("Could not find token '{}'") => ("Kon het token '{}' niet vinden");
    ("Could not create a TTS voice") => ("Kon geen TTS-stem aanmaken");
    ("Error while speaking") => ("Fout bij het uitspreken van de tekst");
    ("Could not set the TTS voice") => ("Kon de TTS-stem niet instellen");
    ("Could not set the speech volume") => ("Kon het spraakvolume niet instellen");
    ("Could not set the speech rate") => ("Kon de spraaksnelheid niet instellen");
    ("Could not obtain the token description") => ("Kon de tokenbeschrijving niet opvragen");
    ("Could not obtain the token ID") => ("Kon de ID van het token niet verkrijgen");
    ("Could not open the dictionary file") => ("Kon het woordenboekbestand niet openen");
    ("Could not parse the dictionary") => ("Kon het woordenboek niet parseren");
    ("Could not write the dictionary file") => ("Kon het woordenboekbestand niet wegschrijven");
    ("Unknown command in 'decoder.scheme': {}") => ("Onbekend commando in 'decoder.scheme': {}");
    ("Sensor index out of range: {}") => ("Sensorindex buiten bereik: {}");
    ("Sensor index in 'decoder.scheme' can not be equal to 'decoder.confirm'") => ("Sensorindex in 'decoder.scheme' mag niet gelijk zijn aan 'decoder.confirm'");
    ("Input defined more than once: [{}]") => ("Invoer meerdere keren gedefinieerd: [{}]");
    ("Error while swapping buffers") => ("Fout bij het uitwisselen van de buffers");
    ("Could not create the window") => ("Kon het venster niet creëren");
    ("Window created. OpenGL version: {}.") => ("Venster gecreëerd. OpenGL-versie: {}.");
    ("Could not enumerate serial ports") => ("Kon seriële poorten niet opvragen");
    ("Application started. Version: {}. Debug mode: {}.") => ("De applicatie is gestart. Versie: {}. Debug-modus: {}.");
    ("Could not create glium renderer") => ("Kon glium renderer niet creëren");
    ("The window was closed.") => ("Het venster werd gesloten.");
    ("The application is shutting down.") => ("De applicatie wordt afgesloten.");
    ("An error has occurred: {}.") => ("Er is een fout opgetreden: {}.");
    ("\n  Caused by:\n    {}.") => ("\n  Veroorzaakt door:\n    {}.");
    ("Could not read the configuration file") => ("Kon het configuratiebestand niet inlezen");
    ("Syntax error: {}.") => ("Syntaxisfout: {}.");
    ("The configuration file contains syntax errors") => ("Het configuratiebestand bevat syntaxisfouten");
    ("The configuration file is invalid") => ("Het configuratiebestand is niet geldig");
    ("commcomm-rs dictionary tool") => ("commcomm-rs woordenboek tool");
    ("Builds a dictionary file from word-frequency file.") => ("Genereert een woordenboekbestand van een woord-frequentielijst.");
    ("Sets a custom output file") => ("Stelt een aangepast uitvoerbestand in");
    ("The input file to use") => ("Het invoerbestand om te gebruiken");
    ("Could not change the sensor setting") => ("Kon sensorinstelling niet wijzigen");
    ("Waiting for Arduino thread to finish.") => ("Bezig met wachten op Arduino-thread.");
    ("Retrying in {} seconds.") => ("Opnieuw proberen over {} seconden.");
    ("The Arduino thread finished.") => ("De Arduino-thread is gestopt.");
    ("Trying to reupload the sketch once.") => ("Eenmalig proberen om de schets opnieuw te uploaden.");
    ("Event buffer is full") => ("Gebeurtenisbuffer is vol");
    ("Serial port could not be opened") => ("Seriële poort kon niet worden geopend");
    ("Unknown error code: {}") => ("Onbekende foutcode: {}");
    ("Could not parse the request") => ("Kon de aanvraag niet parseren");
    ("JSON buffer is full") => ("JSON-buffer is vol");
    ("Request too long") => ("Aanvraagtekst te lang");
    ("Unknown command") => ("Onbekend commando");
    ("Response buffer too small") => ("Antwoordbuffer te klein");
    ("Illegal parameter") => ("Ongeldige parameter");
    ("Arduino is being reset.") => ("Arduino wordt gereset.");
    ("Serial port could not be configured") => ("Seriële poort kon niet worden geconfigureerd");
    ("Waiting for the bootloader port.") => ("Bezig met het wachten op de bootloader-poort.");
    ("Bootloader port found: {}.") => ("Bootloader-poort gevonden: {}.");
    ("Waiting for bootloader port timed out.") => ("Time-out bij het wachten op de bootloader-poort.");
    ("Temporary folder could not be created") => ("Tijdelijke map kon niet worden gemaakt");
    ("Could not write to the AVRDUDE configuration file") => ("Kon het AVRDUDE-configuratiebestand niet wegschrijven");
    ("Could not write to the sketch file") => ("Kon de schets niet wegschrijven");
    ("The AVRDUDE process is being started.") => ("Het AVRDUDE-proces wordt gestart.");
    ("Could not start the AVRDUDE process") => ("Kon het AVRDUDE-proces niet starten");
    ("Error while waiting for the AVRDUDE process") => ("Fout bij het wachten op het AVRDUDE-proces");
    ("Uploading with AVRDUDE failed with error code: {}") => ("Het uploaden met AVRDUDE is mislukt met de foutcode: {}");
    ("Could not kill the AVRDUDE process") => ("Kon het AVRDUDE-proces niet beëindigen");
    ("Waiting for the AVRDUDE process to finish timed out") => ("Time-out bij het wachten op het AVRDUDE-proces");
    ("Waiting for the sketch port.") => ("Bezig met het wachten op de schetspoort.");
    ("Found sketch port: {}.") => ("Schetspoort gevonden: {}.");
    ("Waiting for sketch port timed out.") => ("Time-out bij het wachten op de schetspoort.");
    ("Preparing to upload the sketch.") => ("Bezig met het voorbereiden om de schets te uploaden.");
    ("Upload successful.") => ("Upload succesvol.");
    ("Verifying sketch.") => ("Bezig met het verifiëren van de schets.");
    ("Device information received.") => ("Apparaatinformatie ontvangen.");
    ("Device name: {}.") => ("Apparaatnaam: {}.");
    ("Sketch version: {}.") => ("Schetsversie: {}.");
    ("Timestamp: {}.") => ("Timestamp: {}.");
    ("%Y-%m-%d %H:%M:%S") => ("%d-%m-%Y %H:%M:%S");
    ("Device information does not match") => ("Apparaatinformatie komt niet overeen");
    ("Verification successful.") => ("Verificatie succesvol.");
    ("No device information available; skipping verification.") => ("Geen apparaatinformatie beschikbaar; verificatie wordt overgeslagen.");
    ("Opening sketch port on {}.") => ("Bezig met het openen van de schetspoort op {}.");
    ("Request could not be sent to the Arduino") => ("Aanvraag kon niet worden verzonden naar de Arduino");
    ("Request sent: {}.") => ("Aanvraag verstuurd: {}.");
    ("Arduino response could not be received") => ("Antwoord van de Arduino kon niet worden ontvangen");
    ("Response received: {}.") => ("Antwoord ontvangen: {}.");
    ("Could not parse response") => ("Kon het antwoord niet parseren");
    ("Could not deserialize the response") => ("Kon het antwoord niet deserialiseren");
    ("Serial error") => ("Seriële fout");
    //($text:expr) => (concat!("(Vertaling ontbreekt) ", $text));
}
