
/// Text (typing) related

/// Convert an ASCII char code to a keyboard keycode
pub fn char_to_keycode(chr: u8) -> u8 {
    if chr >= 97 && chr <= 122 {
        chr - 97 + 4
    } else if chr == 44 {
        54
    } else if chr == 46 {
        55
    } else if chr == 32 {
        44
    } else if chr == 10 {
        40
    } else if chr == 39 {
        52
    } else if chr == 33 {
        51 // fake exlamation mark
    } else if chr == 58 {
        51 // fake colon (:)
    } else {
        0
    }
}

pub const TEXT: &str = "
chere mobiliere,

hier soir, tout semblait parfait. j'avais organise un diner avec mes amis, tout etait pret. mais alors que je sortais le gratin du four, j'ai trebuche sur le tapis... le plat s'est envole, et tout s'est renverse sur le sol, y compris mon tapis prefere. heureusement, avec vous, le nettoyage a ete rapide et efficace. merci de m'avoir aide a sauver ma soiree.

chere mobiliere,

ce matin, en me reveillant, je me suis dit que c'etait une belle journee pour une balade en velo. tout se passait bien jusqu'a ce qu'un ecureuil decide de traverser devant moi. pour l'eviter, j'ai freine brusquement et me suis retrouve par terre, avec mon velo en morceaux. heureusement, vous avez ete la pour reparer rapidement mon velo, et l'ecureuil s'en est sorti indemne.

chere mobiliere,

c'etait un jour comme un autre au bureau, jusqu'a ce que je renverse mon verre d'eau sur l'imprimante. l'appareil a fait un drole de bruit et puis plus rien. la panique s'est installee, surtout avec tous les documents importants a imprimer pour une reunion. heureusement, vous etes venus a la rescousse, et en un rien de temps, l'imprimante etait remplacee. merci pour votre rapidite, vous avez sauve ma journee.

chere mobiliere,

hier, j'avais enfin trouve le temps de laver ma voiture. apres une heure de travail acharné, elle brillait comme jamais. mais juste apres avoir termine, une nuée de pigeons est passee au-dessus de moi... et la voiture. heureusement, vous avez ete la pour m'aider a faire le necessaire. merci, vraiment.


chere mobiliere,

ce week-end, j'ai decide de monter un meuble tout seul, sans l'aide des instructions. apres quelques heures de lutte, je me suis retrouve avec une etagere bancale et une vis mysterieuse en trop. le meuble s'est effondre dans la minute. heureusement, vous avez su prendre les choses en main. merci pour votre patience.

chere mobiliere,

c'etait une belle journee de barbecue entre amis. mais lorsque j'ai voulu retourner les brochettes, la grille m'a echappe des mains et tout s'est retrouve par terre. adieu le dejeuner ! heureusement, grace a vous, nous avons pu recommencer sans souci. merci d'avoir sauve notre barbecue.


chere mobiliere,

hier soir, alors que je voulais prendre un bain relaxant, j'ai laissé le robinet ouvert un peu trop longtemps. resultat : une salle de bain inondee, avec de l'eau partout. heureusement, vous avez ete la pour m'aider a reparer les degats. merci d'avoir sauve ma soiree de detente.

chere mobiliere,

l'autre jour, en sortant de la douche, j'ai realise que mon peignoir etait tombe du porte-serviettes. j'ai du me depecher de le ramasser en courant dans l'appartement, en esperant que personne ne passe devant la fenetre. heureusement, tout s'est bien termine, et vous m'avez aide a installer un nouveau porte-serviettes plus solide. merci encore une fois.


chere mobiliere,

ce matin, j'ai voulu prendre un petit-dejeuner au lit pour une fois. mais en voulant me recoucher, j'ai renverse tout mon cafe sur les draps et sur moi-meme. entre le lit trempe et mon pyjama, la journee commencait mal. heureusement, grace a vous, tout a ete nettoye rapidement. merci de m'avoir aide a retablir l'ordre.

chere mobiliere,

hier soir, j'avais prevu une soiree romantique a la maison. tout etait pret : bougies, musique douce... mais en voulant allumer la cheminee pour parfaire l'ambiance, j'ai mal géré et de la fumee a envahi toute la pièce. soiree ratee, mais heureusement, vous avez su nous aider a aerer et nettoyer tout ca. merci d'avoir sauve ce qui restait de l'ambiance.o

chere mobiliere,

l'autre nuit, je me suis reveille un peu deshabillee, apres avoir accidentellement fait tomber toutes les couvertures du lit. en voulant les recuperer, je me suis pris les pieds dedans et me suis retrouvée par terre, completement enchevêtrée dans les draps. heureusement, vous n'etiez pas là pour voir ca, mais vous avez su m'aider a changer mon matelas après l'incident. merci de me sortir des situations les plus embarrassantes.
";
