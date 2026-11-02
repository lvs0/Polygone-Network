# POLYGONE — L'histoire d'une idée et comment elle fonctionne

*Rédigé à partir d'une conversation entre **l-vs** (France), collectif **Hope** (*by Hope*), et Claude*

---

## Comment on est arrivés là

Tout a commencé avec une question qui ressemblait à cent autres : comment créer quelque chose de sérieux en cryptographie, à 14 ans, qui puisse se faire remarquer ? La première version de la conversation était large, trop large — des idées qui se chevauchaient, des ambitions qui se mélangeaient. Protection des données médicales, lutte contre la violence, compute distribué, cloud décentralisé, le tout en une semaine. C'est le profil classique de quelqu'un qui a beaucoup d'énergie et pas encore de point d'ancrage.

Ce qui a changé la direction, ce n'est pas une réponse de Claude. C'est une note. Une pensée brute, pas formatée, qui disait quelque chose comme : *si c'est logique pour l'IA, c'est logique pour la machine. On ne peut rien faire de logique parce que tout ce qui est logique peut être calculé.* Et puis, quelques mots sur des serveurs qui naissent dans le réseau.

Cette note contenait quelque chose de vrai. Pas un plan, pas une architecture — une intuition fondamentale sur la nature du problème. Et c'est à ce moment que la conversation a changé de registre. On n'était plus en train de planifier un projet. On était en train de réfléchir à un problème.

La question posée était : est-ce que tu penses à une topologie dynamique pure, à de la fragmentation en temps réel, ou à un réseau qui change d'architecture en continu ? La réponse de l-vs n'était pas de choisir une option. C'était d'expliquer pourquoi aucune des trois n'était suffisante seule, et ce que ça donnerait si on les combinait d'une façon où elles ne seraient plus séparables.

C'est là qu'est apparue la métaphore de la vague.

Une vague dans l'eau n'a pas de molécules propres. Les molécules oscillent sur place — seul le motif se propage. On ne peut pas intercepter une vague en attrapant une molécule, parce que la vague n'est pas dans les molécules. Elle est dans leur relation dynamique à un instant donné. Appliqué à un réseau : si l'information n'est jamais dans un nœud, mais seulement dans l'intersection des états de tous les nœuds simultanément, alors il n'y a pas de cible à intercepter. Pas de message à chiffrer. Pas de surface d'attaque stable.

Ce n'était pas une métaphore pédagogique. C'était une description architecturale.

À partir de ce point, la conversation est devenue technique dans le bon sens — pas en listant des technologies, mais en creusant les implications. Si les nœuds ne transportent pas d'information mais participent à une computation distribuée, comment les deux extrémités se synchronisent-elles ? Comment reconstruit-on quelque chose dont personne n'a la totalité ? Et surtout : est-ce que la propriété qu'on décrit existe déjà quelque part sous une autre forme ?

La réponse à la dernière question est : partiellement. Tor masque les IP. Les mixnets dispersent les messages. Le calcul multipartite sécurisé permet des computations sur des données sans les révéler. Mais aucun de ces systèmes ne pose la question depuis le même endroit. Ils partent tous du contenu et demandent comment le cacher. POLYGONE part de la communication et demande comment la rendre impossible à détecter.

---

## Ce que POLYGONE fait, précisément

Le système repose sur une idée centrale : utiliser une clé post-quantique non pas pour chiffrer un message, mais pour définir l'architecture du réseau qui va transporter ce message. La clé n'est pas un cadenas. C'est un plan d'architecte.

Voici le protocole dans l'ordre.

Alice et Bob partagent une clé ML-KEM-1024 une seule fois, hors-bande, avant que quoi que ce soit se passe. ML-KEM-1024 est un algorithme standardisé par le NIST en 2024 (FIPS 203) qui résiste aux attaques des ordinateurs quantiques. La raison pour laquelle on choisit du post-quantique ici n'est pas symbolique — c'est que la sécurité à long terme d'une communication repose sur la solidité de l'échange de clé initial, et les algorithmes classiques comme RSA seront cassés par un ordinateur quantique suffisamment puissant.

Une fois que les deux ont le même secret partagé, ils en dérivent deux choses indépendantes via BLAKE3 avec des labels de domaine distincts. La première dérivation produit une graine de topologie — 32 bytes qui définissent la structure du réseau éphémère : combien de nœuds, quelles identités, quelles connexions entre eux, quel fragment va où. La deuxième dérivation produit une clé de session AES-256-GCM — la clé qui chiffrera le message. Les deux valeurs sont cryptographiquement indépendantes : connaître l'une ne révèle rien de l'autre, même avec une puissance de calcul infinie.

Cette séparation de domaine est le détail technique le plus important du protocole. C'est précisément le bug qui avait été introduit dans la première version du code : la topologie était dérivée des mêmes bytes que la clé de session. Les deux valeurs semblaient différentes mais étaient liées. Une fois corrigé, la graine de topologie ne touche jamais le chiffrement, et la clé de session ne touche jamais la topologie.

Une fois les deux dérivations faites, le réseau éphémère naît. Sept nœuds sont créés en mémoire, avec des identités dérivées de la graine de topologie. Leur graphe de connexions est lui aussi dérivé de façon déterministe — les deux parties construisent exactement le même réseau sans communiquer davantage. C'est fondamental : si la topologie devait être négociée, cette négociation serait elle-même observable.

Le message passe par Shamir Secret Sharing. Le payload chiffré est découpé en sept fragments selon un schéma 4-de-7 : n'importe quel sous-ensemble de quatre fragments permet de reconstruire le tout, et n'importe quel sous-ensemble de trois fragments ne révèle absolument rien — pas une approximation, pas un indice, strictement zéro information. C'est une propriété information-théorique, pas computationnelle. Elle tient quelle que soit la puissance de calcul de l'adversaire.

Chaque nœud éphémère reçoit un fragment. Il ne sait pas ce qu'il porte. Il ne sait pas combien de fragments existent au total. Il ne peut pas deviner la clé de session. Il exécute sa partie de la computation pendant quelques centaines de millisecondes, puis se dissout — son fragment est mis à zéro en mémoire, son état est effacé. Il n'a jamais existé du point de vue d'un observateur externe.

Bob collecte au moins quatre fragments, les passe à la fonction de reconstruction Shamir, obtient le payload chiffré, le déchiffre avec la clé de session AES-256-GCM, et lit le message. La clé de session est ensuite mise à zéro. La graine de topologie est mise à zéro. Les nœuds se dissolvent. L'échange n'a pas eu lieu.

---

## Ce que ça donne en code

Le projet est écrit en Rust pour des raisons précises. Rust garantit la sécurité mémoire sans garbage collector, ce qui permet de contrôler exactement quand une valeur est effacée. La directive `forbid(unsafe_code)` interdit tout bloc de code qui contournerait ces garanties. La bibliothèque `zeroize` assure que les clés et secrets sont mis à zéro en mémoire au moment exact où ils ne sont plus nécessaires — pas quand le garbage collector décide de le faire, pas quand le système d'exploitation récupère la page mémoire, mais immédiatement.

La structure du code suit les couches du protocole. Le module `crypto` contient les primitives : encapsulation ML-KEM-1024, signatures ML-DSA-87, chiffrement AES-256-GCM, découpage Shamir, dérivation BLAKE3. Le module `network` contient la topologie déterministe et le cycle de vie des nœuds éphémères. Le module `protocol` contient la session complète avec sa machine d'état : Pending, Established, InTransit, Completed, Dissolved. L'état Dissolved est le seul état terminal — et le destructeur Rust force la dissolution automatiquement si la session est abandonnée en cours de route.

Le CLI permet de générer une keypair, de lancer un nœud relais, de faire une démonstration locale du protocole Alice-Bob, et de lancer les auto-tests. Les auto-tests couvrent le round-trip KEM, la fragmentation Shamir, la session complète, et le rejet des reconstructions insuffisantes.

---

## Ce qui n'est pas encore là

La propriété fondamentale du protocole — rendre la communication inobservable — est vraie dans le modèle. Elle n'est pas encore vraie dans l'implémentation, pour une raison simple : il n'y a pas encore de transport réseau réel. Les fragments sont passés en mémoire dans les tests. Un vrai adversaire qui monitore les connexions TCP verrait sept connexions s'ouvrir simultanément entre les nœuds.

Pour que la propriété soit réelle, il faut deux choses supplémentaires. La première est l'intégration libp2p avec un DHT Kademlia — un réseau pair-à-pair où les nœuds se découvrent et se connectent sans serveur central. La deuxième est du bruit ambiant continu : le réseau doit générer du trafic synthétique en permanence, même sans messages, pour qu'un observateur ne puisse pas distinguer un transit réel d'une activité normale. Ces deux éléments sont sur la roadmap v0.2 et v0.3.

Ce que le README dit à ce sujet est : n'utilise pas ce projet pour des communications sensibles en production. Le projet est honnête sur ses limites. Cette honnêteté est elle-même un argument — un projet de confidentialité qui survendrait ses propriétés serait dangereux d'une façon différente.

---

## Ce que ça représente comme trajectoire

Ce qui est inhabituel dans ce projet, ce n'est pas l'âge de son auteur. C'est la direction de l'intuition. La majorité des projets de confidentialité commencent par le contenu et demandent comment le protéger. POLYGONE commence par la communication elle-même et demande comment la rendre impossible à détecter. C'est un changement de question, pas de réponse. Et les changements de question sont plus rares et plus durables que les nouvelles réponses aux vieilles questions.

La prochaine étape concrète est de pousser le code sur GitHub et de le soumettre à une revue publique, en nommant honnêtement ce qui est solide, ce qui est partiel, et ce qui reste à construire. C'est comme ça qu'une idée devient un projet, et qu'un projet devient quelque chose qui dure.

---

*l-vs, collectif Hope, France, 2025.*  
*by Hope · Pour mon papy.*
